use std::collections::HashSet;
use std::sync::Arc;

use psx_gpu::{Presenter, DISPLAY_HEIGHT};
use web_time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub use winit::keyboard::KeyCode as Key;

const TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);
const MAX_TICKS_PER_FRAME: u32 = 5;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u16>,
}

impl Framebuffer {
    fn new(width: usize) -> Self {
        let height = DISPLAY_HEIGHT as usize;
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub start: (f32, f32),
    pub current: (f32, f32),
}

#[derive(Default)]
pub struct Input {
    pub keys: HashSet<KeyCode>,
    pub pressed: HashSet<KeyCode>,
    pub drag: Option<Drag>,
    pub touches: HashSet<u64>,
    pub tapped: bool,
    pub gamepad: Option<crate::gamepad::Gamepad>,
    pub gamepad_pressed: Vec<usize>,
    pointer_id: Option<u64>,
    mouse_down: bool,
    cursor: (f32, f32),
    surface: (f32, f32),
}

impl Input {
    pub fn held(&self, key: KeyCode) -> bool {
        self.keys.contains(&key)
    }

    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn pad_pressed(&self, button: usize) -> bool {
        self.gamepad_pressed.contains(&button)
    }

    fn poll_gamepad(&mut self) {
        let previous = self.gamepad;
        self.gamepad = crate::gamepad::poll();
        if let Some(pad) = self.gamepad {
            for button in 0..pad.buttons.len() {
                let was = previous.is_some_and(|old| old.held(button));
                if pad.held(button) && !was && !self.gamepad_pressed.contains(&button) {
                    self.gamepad_pressed.push(button);
                }
            }
        }
    }

    pub fn drag_delta(&self) -> Option<(f32, f32)> {
        let scale = self.surface.0.min(self.surface.1).max(1.0);
        self.drag.map(|drag| {
            (
                (drag.current.0 - drag.start.0) / scale,
                (drag.current.1 - drag.start.1) / scale,
            )
        })
    }

    pub fn to_frame(&self, position: (f32, f32), frame_width: u32) -> (i32, i32) {
        let viewport = psx_gpu::letterbox(self.surface.0 as u32, self.surface.1 as u32);
        let x = (position.0 - viewport.x as f32) * frame_width as f32 / viewport.width as f32;
        let y = (position.1 - viewport.y as f32) * DISPLAY_HEIGHT as f32 / viewport.height as f32;
        (x as i32, y as i32)
    }

    fn begin_drag(&mut self, position: (f32, f32)) {
        self.tapped = true;
        self.drag = Some(Drag {
            start: position,
            current: position,
        });
    }

    fn move_drag(&mut self, position: (f32, f32)) {
        if let Some(drag) = &mut self.drag {
            drag.current = position;
        }
    }
}

pub trait Game: 'static {
    fn frame_width(&self) -> u32 {
        320
    }
    fn update(&mut self, input: &Input);
    fn audio(&mut self, _audio: &mut crate::audio::Audio) {}
    fn render(&mut self, framebuffer: &mut Framebuffer);
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    presenter: Presenter,
}

impl Gpu {
    async fn new(window: Arc<Window>, frame_width: u32) -> Self {
        #[allow(unused_mut)]
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        #[cfg(target_arch = "wasm32")]
        {
            let search = web_sys::window()
                .and_then(|window| window.location().search().ok())
                .unwrap_or_default();
            if search.contains("webgl") {
                descriptor.backends = wgpu::Backends::GL;
            }
        }
        let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
        let surface = instance
            .create_surface(window.clone())
            .expect("create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .expect("request adapter");
        log::info!("adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .expect("request device");
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface config");
        let capabilities = surface.get_capabilities(&adapter);
        config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .unwrap_or(config.format);
        surface.configure(&device, &config);
        let presenter = Presenter::new(&device, config.format, frame_width);
        Self {
            window,
            surface,
            device,
            queue,
            config,
            presenter,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn draw(&mut self, framebuffer: &Framebuffer) {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        self.presenter
            .set_frame_width(&self.device, framebuffer.width as u32);
        self.presenter
            .upload_bgr555(&self.queue, &framebuffer.pixels, framebuffer.width);
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.presenter.present(
            &self.queue,
            &mut encoder,
            &view,
            self.config.width,
            self.config.height,
        );
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(surface_texture);
    }
}

struct App<G: Game> {
    game: G,
    title: String,
    proxy: EventLoopProxy<Gpu>,
    gpu: Option<Gpu>,
    framebuffer: Framebuffer,
    input: Input,
    audio: crate::audio::Audio,
    last_frame: Instant,
    accumulator: Duration,
}

impl<G: Game> App<G> {
    fn step(&mut self) {
        let now = Instant::now();
        self.accumulator += now - self.last_frame;
        self.last_frame = now;
        let mut ticks = 0;
        while self.accumulator >= TICK {
            self.accumulator -= TICK;
            ticks += 1;
            if ticks > MAX_TICKS_PER_FRAME {
                self.accumulator = Duration::ZERO;
                break;
            }
            self.input.poll_gamepad();
            self.game.update(&self.input);
            self.input.pressed.clear();
            self.input.gamepad_pressed.clear();
            self.input.tapped = false;
        }
        let width = self.game.frame_width() as usize;
        if self.framebuffer.width != width {
            self.framebuffer = Framebuffer::new(width);
        }
        self.game.render(&mut self.framebuffer);
        self.game.audio(&mut self.audio);
    }
}

#[cfg(target_arch = "wasm32")]
impl<G: Game> App<G> {
    fn match_canvas_size(&mut self) {
        use winit::platform::web::WindowExtWebSys;
        let Some(gpu) = &mut self.gpu else { return };
        let Some(canvas) = gpu.window.canvas() else {
            return;
        };
        let ratio = web_sys::window()
            .map(|window| window.device_pixel_ratio())
            .unwrap_or(1.0);
        let width = (canvas.client_width() as f64 * ratio).round().max(1.0) as u32;
        let height = (canvas.client_height() as f64 * ratio).round().max(1.0) as u32;
        if width != gpu.config.width || height != gpu.config.height {
            canvas.set_width(width);
            canvas.set_height(height);
            gpu.resize(width, height);
        }
        self.input.surface = (width as f32, height as f32);
    }
}

impl<G: Game> ApplicationHandler<Gpu> for App<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        #[allow(unused_mut)]
        let mut attributes = Window::default_attributes().with_title(&self.title);
        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_inner_size(winit::dpi::LogicalSize::new(960, 720));
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes = attributes.with_append(true);
        }
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;
            if let Some(canvas) = window.canvas() {
                let style = canvas.style();
                let _ = style.set_property("width", "100vw");
                let _ = style.set_property("height", "100dvh");
                let _ = style.set_property("display", "block");
                let _ = style.set_property("touch-action", "none");
            }
        }
        let frame_width = self.game.frame_width();
        let proxy = self.proxy.clone();
        let future = async move {
            let gpu = Gpu::new(window, frame_width).await;
            let _ = proxy.send_event(gpu);
        };
        #[cfg(not(target_arch = "wasm32"))]
        pollster::block_on(future);
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(future);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, gpu: Gpu) {
        let mut gpu = gpu;
        if self.input.surface.0 >= 1.0 && self.input.surface.1 >= 1.0 {
            gpu.resize(self.input.surface.0 as u32, self.input.surface.1 as u32);
        }
        self.input.surface = (gpu.config.width as f32, gpu.config.height as f32);
        gpu.window.request_redraw();
        self.gpu = Some(gpu);
        self.last_frame = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.input.surface = (size.width as f32, size.height as f32);
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.audio.unlock();
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat {
                                self.input.pressed.insert(code);
                            }
                            self.input.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.input.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.input.keys.clear();
                self.input.touches.clear();
                self.input.pointer_id = None;
                self.input.drag = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = (position.x as f32, position.y as f32);
                self.input.cursor = position;
                if self.input.mouse_down {
                    self.input.move_drag(position);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.input.mouse_down = state == ElementState::Pressed;
                self.audio.unlock();
                if self.input.mouse_down {
                    let cursor = self.input.cursor;
                    self.input.begin_drag(cursor);
                } else {
                    self.input.drag = None;
                }
            }
            WindowEvent::Touch(touch) => {
                self.audio.unlock();
                let position = (touch.location.x as f32, touch.location.y as f32);
                match touch.phase {
                    TouchPhase::Started => {
                        self.input.touches.insert(touch.id);
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.input.touches.remove(&touch.id);
                    }
                    _ => {}
                }
                match touch.phase {
                    TouchPhase::Started if self.input.pointer_id.is_none() => {
                        self.input.pointer_id = Some(touch.id);
                        self.input.begin_drag(position);
                    }
                    TouchPhase::Moved if self.input.pointer_id == Some(touch.id) => {
                        self.input.move_drag(position);
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled
                        if self.input.pointer_id == Some(touch.id) =>
                    {
                        self.input.pointer_id = None;
                        self.input.drag = None;
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if self.gpu.is_none() {
                    return;
                }
                #[cfg(target_arch = "wasm32")]
                self.match_canvas_size();
                self.step();
                if let Some(gpu) = &mut self.gpu {
                    gpu.draw(&self.framebuffer);
                    gpu.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

pub fn run<G: Game>(title: &str, game: G) {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(log::Level::Info);
    }

    let event_loop = EventLoop::<Gpu>::with_user_event()
        .build()
        .expect("event loop");
    let framebuffer = Framebuffer::new(game.frame_width() as usize);
    let app = App {
        game,
        title: title.to_string(),
        proxy: event_loop.create_proxy(),
        gpu: None,
        framebuffer,
        input: Input::default(),
        audio: crate::audio::Audio::default(),
        last_frame: Instant::now(),
        accumulator: Duration::ZERO,
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("run app");
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}
