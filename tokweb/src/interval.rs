#[wasm_bindgen]
extern "C" {
    fn setInterval(closure: &Closure<dyn FnMut()>, millis: u32) -> f64;
    fn cancelInterval(token: f64);
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct LeakyInterval {
    token: f64,
}

impl LeakyInterval {
    pub fn new<F: 'static>(duration: std::time::Duration, f: F) -> LeakyInterval
    where
        F: FnMut(),
    {
        let closure = Closure::new(f);
        let millis = duration.as_millis() as u32;

        let token = unsafe { setInterval(&closure, millis) };
        closure.forget();

        LeakyInterval { token }
    }
}

impl Drop for LeakyInterval {
    fn drop(&mut self) {
        unsafe { cancelInterval(self.token); }
    }
}