use std::sync::Arc;

#[allow(unused_imports)]
use wasm_bindgen::{prelude::wasm_bindgen, UnwrapThrowExt};

extern crate console_error_panic_hook;

use wgpu::{Adapter, Device, Instance, Queue, RenderPipeline, Surface, SurfaceConfiguration};
use winit::{application::ApplicationHandler, event_loop::EventLoopProxy, window::Window};

#[allow(dead_code)]
struct GfxState {
    window: Arc<Window>,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    instance: Instance,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    render_pipeline: RenderPipeline,
}

impl GfxState {
    fn new(window: Arc<Window>, instance: Instance, surface: Surface<'static>, surface_config: SurfaceConfiguration, adapter: Adapter, device: Device, queue: Queue, render_pipeline: RenderPipeline) -> Self {
        Self {
            window,
            instance,
            surface,
            surface_config,
            adapter,
            device,
            queue,
            render_pipeline,
        }
    }
}

struct App {
    gfx_state: GfxState,
}

impl App {
    async fn new(
        window: Arc<Window>,
    ) -> Self {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ).await.unwrap();

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            desired_maximum_frame_latency: 2,
        };
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            surface.configure(&device, &surface_config);
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let gfx_state = GfxState::new(window, instance, surface, surface_config, adapter, device, queue, render_pipeline);

        Self {
            gfx_state,
        }
    }


    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.gfx_state.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
            .. Default::default()
        });
        let mut encoder = self.gfx_state.device.create_command_encoder(&Default::default());

        {
            let clear_color = wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 1.0,
            };

            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            };
            let render_pass_desc = wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            };
            let mut _render_pass = encoder.begin_render_pass(&render_pass_desc);
            _render_pass.set_pipeline(&self.gfx_state.render_pipeline);
            _render_pass.draw(0..3, 0..1);
        }

        self.gfx_state.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.gfx_state.surface_config.width = size.width;
            self.gfx_state.surface_config.height = size.height;
            self.gfx_state.surface.configure(&self.gfx_state.device, &self.gfx_state.surface_config);
        }
    }
}

enum CustomEvent {
    Initialized(App),
}

enum AppState {
    Uninitialized(EventLoopProxy<CustomEvent>),
    Initialized(App),
}

impl ApplicationHandler<CustomEvent> for AppState {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        match self {
            AppState::Uninitialized(event_loop_proxy) => {
                let window_attrs = Window::default_attributes();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
                    let app = pollster::block_on(App::new(window));

                    assert!(event_loop_proxy.send_event(CustomEvent::Initialized(app)).is_ok());
                }

                #[cfg(target_arch = "wasm32")]
                {
                    use winit::dpi::PhysicalSize;
                    use winit::platform::web::WindowAttributesExtWebSys;

                    let window_attrs = window_attrs.with_append(true);
                    let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

                    let _ = window.request_inner_size(PhysicalSize::new(450, 400));

                    let event_loop_proxy = event_loop_proxy.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let app = App::new(window).await;
                        assert!(event_loop_proxy.send_event(CustomEvent::Initialized(app)).is_ok());
                    });
                }
            }
            AppState::Initialized(_) => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let app = match self {
            AppState::Initialized(app) => app,
            AppState::Uninitialized(_) => return,
        };

        match event {
            winit::event::WindowEvent::Resized(size) => app.resize(size),
            winit::event::WindowEvent::RedrawRequested => {
                #[cfg(target_arch = "wasm32")]
                {
                    use web_sys::console;
        
                    console::log_1(&"Redraw requested.".into());
                }
                match app.render() {
                    Ok(_) => {}
                    Err(e) => {
                        #[cfg(target_arch = "wasm32")]
                        {
                            use web_sys::console;

                            console::log_1(&format!("Error during rendering: {:?}", e).into());
                        }
                        eprintln!("Error during rendering: {:?}", e);
                    }
                }
            },
            winit::event::WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn user_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _user_event: CustomEvent,
    ) {
        match _user_event {
            CustomEvent::Initialized(app) => {
                take_mut::take(self, |state| match state {
                    AppState::Uninitialized(_) => AppState::Initialized(app),
                    AppState::Initialized(_) => state,
                });
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;

        console_error_panic_hook::set_once();
    }

    let event_loop = winit::event_loop::EventLoop::with_user_event().build().unwrap();
    let mut app = AppState::Uninitialized(event_loop.create_proxy());

    #[cfg(not(target_arch = "wasm32"))]
    {
        event_loop.run_app(&mut app).unwrap();
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        
        event_loop.spawn_app(app);
    }
}

