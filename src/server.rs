pub struct Ctx {
    pub width: i32,
    pub height: i32,
}

fn png_screen(ctx: &Ctx) -> Vec<u8> {
    let view = crate::input::view();
    let data = crate::fetch::get_data(&view);
    let total = crate::render::total_pages(&data, ctx.height, &view).max(1);
    let svg = crate::render::render_svg(&data, ctx.width, ctx.height, &view, crate::input::clamp_page(total));
    crate::render::render_pixmap(&svg).encode_png().unwrap_or_default()
}

pub fn serve(host: &str, port: u16, ctx: Ctx) {
    let addr = format!("{host}:{port}");
    let server = match tiny_http::Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("HTTP server error: {e}");
            return;
        }
    };
    eprintln!("HTTP Server running on http://{addr}");
    for req in server.incoming_requests() {
        let url = req.url().to_string();
        let (path, query) = match url.split_once('?') { Some((p, q)) => (p, q), None => (url.as_str(), "") };
        let q = |k: &str| {
            query.split('&').find_map(|kv| {
                let (kk, vv) = kv.split_once('=')?;
                (kk == k).then(|| vv.to_string())
            })
        };
        match path {
            "/health" => {
                req.respond(tiny_http::Response::from_string("ok")).ok();
            }
            "/api" => {
                let data = crate::fetch::get_data(&crate::input::view());
                let body = serde_json::to_string(&data).unwrap_or("[]".to_string());
                req.respond(
                    tiny_http::Response::from_string(body)
                        .with_header(h("Content-Type: application/json; charset=utf-8")),
                )
                .ok();
            }
            "/switch" => {
                if let Some(v) = q("view") {
                    crate::input::set_view(&v);
                }
                redirect(req, "/");
            }
            "/style" => {
                match q("mode") {
                    Some(m) => crate::input::set_style(&m),
                    None => {
                        crate::input::next_style();
                    }
                }
                redirect(req, "/");
            }
            "/exit" => {
                req.respond(tiny_http::Response::from_string("Exiting...")).ok();
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    crate::input::quit_app();
                });
            }
            "/screen.png" => {
                let png = png_screen(&ctx);
                req.respond(
                    tiny_http::Response::from_data(png)
                        .with_header(h("Content-Type: image/png"))
                        .with_header(h("Cache-Control: no-cache")),
                )
                .ok();
            }
            _ => {
                if let Some(v) = q("view") {
                    crate::input::set_view(&v);
                }
                let view = crate::input::view();
                let tabs = crate::config::tab_modes();
                let mut links = String::new();
                for t in &tabs {
                    links.push_str(&format!("<a href=\"/switch?view={t}\">[{t}]</a> "));
                }
                let html = format!(
                    "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
                     <meta http-equiv=\"refresh\" content=\"60\">\
                     <meta name=\"viewport\" content=\"width=1072, initial-scale=1\">\
                     <title>kkanpan {view}</title></head>\
                     <body style=\"background:#eee;text-align:center\">\
                     <div>{links}<a href=\"/style\">[style:{freight}]</a> \
                     <a href=\"/exit\">[exit]</a></div>\
                     <img src=\"/screen.png\" style=\"max-width:1072px;width:100%;background:#fff\">\
                     </body></html>",
                    freight = crate::input::style_label()
                );
                req.respond(
                    tiny_http::Response::from_string(html)
                        .with_header(h("Content-Type: text/html; charset=utf-8"))
                        .with_header(h("Cache-Control: no-cache")),
                )
                .ok();
            }
        }
    }
}

fn h(s: &str) -> tiny_http::Header {
    s.parse().unwrap()
}

fn redirect(req: tiny_http::Request, to: &str) {
    req.respond(tiny_http::Response::empty(302).with_header(h(&format!("Location: {to}")))).ok();
}
