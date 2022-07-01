use js_sys::encode_uri;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use wiki::Site;
use yew::prelude::*;



#[function_component(App)]
fn app() -> Html {
    let window = web_sys::window().unwrap();
    let res = use_state(|| String::new());
    let timeout = use_state(|| -1);
    let input_ref = use_node_ref();
    let site = Site::enwiki();
    let keyup = {
        use wasm_bindgen::closure::Closure;
        let input_ref = input_ref.clone();
        let res = res.clone();
        let c = Closure::once(Box::new(move || {
            wasm_bindgen_futures::spawn_local(async move {
                let p = encode_uri(&input_ref.cast::<HtmlInputElement>().unwrap().value());
                let s = p.as_string().unwrap();
                let t = reqwest::get(format!("https://en.wikipedia.org/w/api.php?action=query&format=json&origin=*&list=search&srsearch={s}&srwhat=text&srprop=redirecttitle")).await.unwrap();
                let t = t.text().await.unwrap();
                res.set(t);
            })
        }) as Box<dyn FnOnce()>);
        let c = Box::leak(Box::new(c));
        Callback::from(move |_| {
            if *timeout != -1 {
                window.clear_timeout_with_handle(*timeout);
            }
            let t = window
                .set_timeout_with_callback(c.as_ref().unchecked_ref())
                .unwrap();
            timeout.set(t)
        })
    };

    html! {
        <>
            <input onkeyup={keyup} ref={input_ref}/>
            <p>{&*res}</p>
        </>
    }
}

fn inputevent(e: KeyboardEvent) {
    println!("lol")
}

fn main() {
    yew::start_app::<App>();
}
