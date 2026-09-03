use crate::app::App;
use crate::layout::Style;
use image::ImageFormat;
use std::io::Cursor;
use std::sync::Arc;
use tiny_http::{Header, Response, Server};

fn query_param(url: &str, key: &str) -> Option<String> {
    url.split('?').nth(1).unwrap_or("").split('&').find_map(|kv| {
        let mut it = kv.splitn(2, '=');
        if it.next() == Some(key) {
            it.next().map(|v| v.to_string())
        } else {
            None
        }
    })
}

fn redirect(to: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("")
        .with_status_code(302)
        .with_header(
            Header::from_bytes(&b"Location"[..], to.as_bytes()).unwrap(),
        )
}

pub fn serve(app: Arc<App>, host: String, port: u16) {
    let addr = format!("{host}:{port}");
    let Ok(server) = Server::http(&addr) else {
        eprintln!("[http] cannot listen on {addr}");
        return;
    };
    eprintln!("[http] running on http://{addr}");
    for req in server.incoming_requests() {
        let url = req.url().to_string();
        let path = url.split('?').next().unwrap_or("/").to_string();
        let resp: Response<Cursor<Vec<u8>>> = match path.as_str() {
            "/health" => Response::from_string("ok"),
            "/api" => {
                let d = app.data();
                let body = serde_json::to_string(&d).unwrap_or("[]".into());
                Response::from_string(body).with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
                        .unwrap(),
                )
            }
            "/switch" => {
                if let Some(v) = query_param(&url, "view") {
                    if !v.is_empty() {
                        app.set_view(&v);
                        eprintln!("[http] switch view -> {v}");
                    }
                }
                redirect("/")
            }
            "/style" => {
                if let Some(m) = query_param(&url, "mode") {
                    match m.as_str() {
                        "large" => app.set_style(Style::Large),
                        "normal" => app.set_style(Style::Normal),
                        _ => {}
                    }
                } else {
                    app.next_style();
                }
                redirect("/")
            }
            "/exit" => {
                let _ = req.respond(Response::from_string("Exiting kkanpan..."));
                app.quit();
                return;
            }
            "/screen.svg" => {
                let d = app.data();
                let svg = app.render_svg_current(&d);
                Response::from_string(svg).with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"image/svg+xml; charset=utf-8"[..])
                        .unwrap(),
                )
            }
            "/screen.png" => {
                let d = app.data();
                let (img, _, _) = app.render_current(&d);
                let mut buf = vec![];
                if img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png).is_ok() {
                    Response::from_data(buf).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..]).unwrap(),
                    )
                } else {
                    Response::from_string("render failed").with_status_code(500)
                }
            }
            _ => {
                if path.ends_with(".svg") {
                    redirect("/screen.svg")
                } else {
                    if let Some(v) = query_param(&url, "view") {
                        if !v.is_empty() {
                            app.set_view(&v);
                        }
                    }
                    let d = app.data();
                    let svg = app.render_svg_current(&d);
                    let html = format!(
                        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"60\"><meta name=\"viewport\" content=\"width=1072, initial-scale=1\"><title>kkanpan</title><style>*{{margin:0;padding:0;box-sizing:border-box}}body{{background:#eee;display:flex;justify-content:center;padding:8px}}svg{{max-width:1072px;width:100%;height:auto;background:#fff}}</style></head><body>{svg}</body></html>"
                    );
                    Response::from_string(html).with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                            .unwrap(),
                    )
                }
            }
        };
        let _ = req.respond(resp);
    }
}
