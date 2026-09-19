//! The gpui-cn gallery on iOS.
//!
//! GPUI's iOS platform (`gpui-mobile`) draws into a UIKit view controller
//! and leaves the application, the window, and the frame clock to a host.
//! gpui-kit's own example writes that host in Swift. This binary is the
//! same host written with `objc2`, so the application is Rust from `main`
//! down. `ios/run.sh` builds it, wraps it in an app bundle, and launches it
//! in the simulator.

#[cfg(target_os = "ios")]
fn main() {
    ios::main();
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!(
        "gpui-cn-story-ios runs on iOS. Build it with ios/run.sh or --target aarch64-apple-ios-sim."
    );
}

#[cfg(target_os = "ios")]
mod ios {
    use std::{
        cell::{Cell, RefCell},
        ffi::{c_char, c_int, c_void},
        rc::Rc,
    };

    use gpui_cn::Theme;
    use gpui_kit::{
        App, AppContext as _, Application, ApplicationHandle, Context, Entity, IntoElement,
        ParentElement as _, Render, Styled as _, Window, WindowOptions, div, px,
    };
    use gpui_mobile::ios::ffi;
    use objc2::{
        ClassType, DefinedClass, MainThreadOnly, class, define_class, msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyObject, Bool, NSObject, NSObjectProtocol},
        sel,
    };
    use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes, NSString};

    /// The gallery kept out of the status bar, the home indicator, and the
    /// camera. GPUI's iOS platform reports the safe area through
    /// `gpui_mobile::safe_area_insets`, not through the window, so the
    /// host applies it. A layout change re-renders the window, which reads
    /// the insets again.
    struct SafeArea {
        gallery: Entity<gpui_cn_story::Gallery>,
    }

    impl Render for SafeArea {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let (top, bottom, left, right) = gpui_mobile::safe_area_insets();
            div()
                .size_full()
                .pt(px(top))
                .pb(px(bottom))
                .pl(px(left))
                .pr(px(right))
                .child(self.gallery.clone())
        }
    }

    // The application delegate. Since the iOS 26 SDK an application must
    // use the scene life cycle, so this does nothing but exist; the scene
    // delegate below does the work.
    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "GpuiCnAppDelegate"]
        struct AppDelegate;

        unsafe impl NSObjectProtocol for AppDelegate {}

        impl AppDelegate {
            #[unsafe(method(application:didFinishLaunchingWithOptions:))]
            fn did_finish_launching(&self, _app: &AnyObject, _options: *const AnyObject) -> Bool {
                Bool::YES
            }
        }
    );

    // UIKit requires the scene delegate class to adopt these protocols by
    // name. Their methods are all optional, so the declarations are empty.
    objc2::extern_protocol!(
        unsafe trait UISceneDelegate: NSObjectProtocol {}
    );
    objc2::extern_protocol!(
        unsafe trait UIWindowSceneDelegate: UISceneDelegate {}
    );

    /// What the scene delegate keeps: GPUI itself, the UIKit window it made,
    /// GPUI's window, and the display link that gives GPUI frames.
    #[derive(Default)]
    struct SceneIvars {
        /// UIKit owns the run loop, so GPUI lives here for the
        /// application's lifetime.
        application: RefCell<Option<ApplicationHandle>>,
        ui_window: RefCell<Option<Retained<AnyObject>>>,
        window: Cell<*mut c_void>,
        display_link: RefCell<Option<Retained<AnyObject>>>,
        frames: RefCell<FrameStats>,
    }

    /// What frames cost, printed to the console every two seconds while
    /// the display link runs, with `GPUI_CN_FRAME_STATS` set. A paused
    /// link prints nothing, which is itself the idle measurement.
    #[derive(Default)]
    struct FrameStats {
        enabled: Option<bool>,
        samples: Vec<f64>,
        since: Option<std::time::Instant>,
    }

    impl FrameStats {
        fn record(&mut self, cost: std::time::Duration) {
            let enabled = *self
                .enabled
                .get_or_insert_with(|| std::env::var_os("GPUI_CN_FRAME_STATS").is_some());
            if !enabled {
                return;
            }
            let now = std::time::Instant::now();
            let since = *self.since.get_or_insert(now);
            self.samples.push(cost.as_secs_f64() * 1000.);
            let elapsed = now.duration_since(since).as_secs_f64();
            if elapsed >= 2. {
                self.samples.sort_by(|a, b| a.total_cmp(b));
                let at = |q: f64| self.samples[((self.samples.len() - 1) as f64 * q) as usize];
                let total: f64 = self.samples.iter().sum();
                println!(
                    "frames: {} in {elapsed:.1} s ({:.0}/s), {:.2} ms p50, {:.2} ms p95, {:.2} ms max, {:.0}% of the time",
                    self.samples.len(),
                    self.samples.len() as f64 / elapsed,
                    at(0.5),
                    at(0.95),
                    at(1.),
                    total / (elapsed * 1000.) * 100.
                );
                self.samples.clear();
                self.since = Some(now);
            }
        }
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "GpuiCnSceneDelegate"]
        #[ivars = SceneIvars]
        struct SceneDelegate;

        unsafe impl NSObjectProtocol for SceneDelegate {}
        unsafe impl UISceneDelegate for SceneDelegate {}
        unsafe impl UIWindowSceneDelegate for SceneDelegate {}

        impl SceneDelegate {
            // UIKit makes the delegate with `alloc` and `init`, so `init` is
            // where the Rust fields get their first values.
            #[unsafe(method_id(init))]
            fn init(this: Allocated<Self>) -> Retained<Self> {
                let this = this.set_ivars(SceneIvars::default());
                unsafe { msg_send![super(this), init] }
            }

            #[unsafe(method(scene:willConnectToSession:options:))]
            fn will_connect(&self, scene: &AnyObject, _session: &AnyObject, _options: &AnyObject) {
                // GPUI draws into a view controller that this window hosts.
                // The platform would make its own UIWindow otherwise, which
                // the scene life cycle does not allow.
                ffi::gpui_ios_set_embedded();
                // This is `gpui_mobile::ios::ffi::run_app` with one change:
                // the application gets the Lucide icons, which `run_app`
                // has no way to pass.
                ffi::gpui_ios_initialize();
                let platform = Rc::new(gpui_mobile::IosPlatform::new());
                let application = Application::with_platform(platform)
                    .with_assets(gpui_kit::assets::Assets)
                    .run_embedded(|cx: &mut App| {
                        gpui_kit::init(cx);
                        gpui_cn::init(cx);
                        // The status bar draws over the window, so its
                        // text follows the theme: white on dark, black
                        // on light.
                        let status_bar = |cx: &App| {
                            gpui_mobile::ios::set_status_bar_style(if Theme::global(cx).is_dark() {
                                gpui_mobile::StatusBarContentStyle::Light
                            } else {
                                gpui_mobile::StatusBarContentStyle::Dark
                            });
                        };
                        status_bar(cx);
                        cx.observe_global::<Theme>(move |cx| status_bar(cx)).detach();
                        cx.open_window(
                            WindowOptions {
                                window_bounds: None,
                                ..Default::default()
                            },
                            |window, cx| {
                                let gallery =
                                    cx.new(|cx| gpui_cn_story::Gallery::new(window, cx));
                                let safe = cx.new(|_| SafeArea { gallery });
                                cx.new(|cx| gpui_cn::Root::new(safe, window, cx))
                            },
                        )
                        .expect("open the gallery window");
                        cx.activate(true);
                    });
                *self.ivars().application.borrow_mut() = Some(application);
                // On iOS `run_embedded` stores the launch callback; this
                // runs it, which opens the window.
                ffi::gpui_ios_did_finish_launching(std::ptr::null_mut());

                let window = ffi::gpui_ios_get_window();
                self.ivars().window.set(window);
                if window.is_null() {
                    log::error!("gpui-cn-story-ios: GPUI made no window");
                    return;
                }
                let controller = unsafe { ffi::gpui_ios_view_controller(window) } as *mut AnyObject;
                let ui_window: Retained<AnyObject> = unsafe {
                    let window: *mut AnyObject = msg_send![class!(UIWindow), alloc];
                    let window: *mut AnyObject = msg_send![window, initWithWindowScene: scene];
                    Retained::from_raw(window).expect("a UIWindow")
                };
                unsafe {
                    let _: () = msg_send![&*ui_window, setRootViewController: controller];
                    let _: () = msg_send![&*ui_window, makeKeyAndVisible];
                    // Publish the view's size and the first frame together,
                    // as gpui-kit's Swift host does.
                    ffi::gpui_ios_layout_view(window);
                }
                ffi::gpui_ios_request_frame(window);
                *self.ivars().ui_window.borrow_mut() = Some(ui_window);

                // The display link gives GPUI a frame per vsync. GPUI says
                // whether it wants another; the link pauses in between and
                // the waker resumes it, so an idle screen costs nothing.
                let link: Retained<AnyObject> = unsafe {
                    msg_send![class!(CADisplayLink), displayLinkWithTarget: self, selector: sel!(renderFrame:)]
                };
                let run_loop = NSRunLoop::mainRunLoop();
                unsafe {
                    let _: () = msg_send![&*link, addToRunLoop: &*run_loop, forMode: NSRunLoopCommonModes];
                }
                *self.ivars().display_link.borrow_mut() = Some(link);
                let context: *const SceneDelegate = self;
                ffi::gpui_ios_set_frame_waker(window, Some(resume_frames), context as *mut c_void);
            }

            #[unsafe(method(renderFrame:))]
            fn render_frame(&self, _link: &AnyObject) {
                let window = self.ivars().window.get();
                if window.is_null() {
                    return;
                }
                let start = std::time::Instant::now();
                let wants_more = ffi::gpui_ios_request_frame(window);
                self.ivars().frames.borrow_mut().record(start.elapsed());
                if !wants_more {
                    self.set_paused(true);
                }
            }

            #[unsafe(method(sceneDidBecomeActive:))]
            fn did_become_active(&self, _scene: &AnyObject) {
                ffi::gpui_ios_did_become_active(std::ptr::null_mut());
            }

            #[unsafe(method(sceneWillResignActive:))]
            fn will_resign_active(&self, _scene: &AnyObject) {
                ffi::gpui_ios_will_resign_active(std::ptr::null_mut());
            }

            #[unsafe(method(sceneDidEnterBackground:))]
            fn did_enter_background(&self, _scene: &AnyObject) {
                ffi::gpui_ios_did_enter_background(std::ptr::null_mut());
                self.set_paused(true);
            }

            #[unsafe(method(sceneWillEnterForeground:))]
            fn will_enter_foreground(&self, _scene: &AnyObject) {
                ffi::gpui_ios_will_enter_foreground(std::ptr::null_mut());
                self.set_paused(false);
            }
        }
    );

    impl SceneDelegate {
        fn set_paused(&self, paused: bool) {
            if let Some(link) = self.ivars().display_link.borrow().as_ref() {
                unsafe {
                    let _: () = msg_send![&**link, setPaused: paused];
                }
            }
        }
    }

    /// GPUI calls this on the main thread when it wants a frame after the
    /// link paused.
    unsafe extern "C" fn resume_frames(context: *mut c_void) {
        let delegate = unsafe { &*(context as *const SceneDelegate) };
        delegate.set_paused(false);
    }

    unsafe extern "C" {
        fn UIApplicationMain(
            argc: c_int,
            argv: *const *const c_char,
            principal_class_name: *const NSString,
            delegate_class_name: *const NSString,
        ) -> c_int;
    }

    /// Routes the `log` crate and panics to stdout, which
    /// `xcrun simctl launch --console` shows.
    struct StdoutLogger;

    impl log::Log for StdoutLogger {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            println!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
        fn flush(&self) {}
    }

    pub fn main() {
        let _ = log::set_logger(&StdoutLogger).map(|()| log::set_max_level(log::LevelFilter::Info));
        std::panic::set_hook(Box::new(|info| println!("PANIC: {info}")));
        let args: Vec<std::ffi::CString> = std::env::args()
            .map(|arg| std::ffi::CString::new(arg).expect("an argument without NUL"))
            .collect();
        let argv: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
        let delegate = NSString::from_str("GpuiCnAppDelegate");
        // Touch both classes so the Objective-C runtime has registered
        // them before UIKit looks them up by name.
        let _ = AppDelegate::class();
        let _ = SceneDelegate::class();
        unsafe {
            UIApplicationMain(
                argv.len() as c_int,
                argv.as_ptr(),
                std::ptr::null(),
                &*delegate,
            );
        }
    }
}
