/*
 * @Description:
 * @Author: 李昶
 * @Date: 2022-10-22 17:18:57
 * @LastEditors: 李昶
 * @LastEditTime: 2022-10-24 22:45:30
 * @Profile: 一个比较废柴的前端开发
 */
use anyhow::Result;
use axum::{
    extract::{Extension, Path},
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::get,
    Router,
};
use bytes::Bytes;
use lru::LruCache;
use percent_encoding::{percent_decode_str, percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::Arc,
};

use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tracing::{info, instrument};

// 引入protobuf生成的代码
mod pb;

use pb::*;

// 参数使用serde做Deserialize，axum可以自动识别并解析
#[derive(Deserialize)]
struct Params {
    url: String,
}

type Cache = Arc<Mutex<LruCache<u64, Bytes>>>;

// 解析出来的图片处理的参数
// struct ImageSpec {
//     specs: Vec<Spec>,
// }

// // 每个参数是支持的某种方式
// enum Spec {
//     Resize(Resize),
//     Crop(Crop),
// }

// // 处理图片的resize
// struct Resize {
//     width: u32,
//     height: u32,
// }

// message ImageSpec { repeated Spec specs = 1; }

// message Spec {
//     oneof data {
//         Resize resize = 1;
//         Crop crop = 2;
//     }
// }
#[tokio::main]
async fn main() {
    // 初始化tracing
    tracing_subscriber::fmt::init();
    let cache: Cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())));

    // 构建路由
    let app = Router::new()
        // Get /image 会执行generate函数，并把spec和url传递过去
        .route("/image/:spec/:url", get(generate))
        .layer(ServiceBuilder::new().layer(Extension(cache)).into_inner());

    // 运行web服务器
    let addr = "127.0.0.1:3000".parse().unwrap();

    print_test_url("https://images.pexels.com/photos/1562477/pexels-photo-1562477.jpeg?auto=compress&cs=tinysrgb&dpr=3&h=750&w=1260");

    info!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// 解析参数
async fn generate(
    Path(Params { url }): Path<Params>,
    Extension(cache): Extension<Cache>,
) -> Result<(HeaderMap, Vec<u8>), StatusCode> {
    let url: &str = &percent_decode_str(&url).decode_utf8_lossy();

    let data = retrieve_image(&url, cache)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // 处理图片

    let mut headers = HeaderMap::new();

    headers.insert("content-type", HeaderValue::from_static("image/jpeg"));
    Ok((headers, data.to_vec()))
}

#[instrument(level = "info", skip(cache))]
async fn retrieve_image(url: &str, cache: Cache) -> Result<Bytes> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let key = hasher.finish();

    let g = &mut cache.lock().await;
    let data = match g.get(&key) {
        Some(v) => {
            info!("Match cache {}", key);
            v.to_owned()
        }
        None => {
            info!("Retrieve url");
            let resp = reqwest::get(url).await?;
            let data = resp.bytes().await?;
            g.put(key, data.clone());
            data
        }
    };

    Ok(data)
}

fn print_test_url(url: &str) {
    use std::borrow::Borrow;
    let spec1 = Spec::new_resize(500, 800, resize::SampleFilter::CatmullRom);
    let spec2 = Spec::new_watermark(20, 20);
    let spec3 = Spec::new_filter(filter::Filter::Marine);
    let image_spec = ImageSpec::new(vec![spec1, spec2, spec3]);
    let s: String = image_spec.borrow().into();
    let test_image = percent_encode(url.as_bytes(), NON_ALPHANUMERIC).to_string();
    println!("test url: http://localhost:3000/image/{}/{}", s, test_image);
}
