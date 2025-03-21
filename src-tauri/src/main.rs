// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::command;
use tauri::State;
use tauri::Emitter;
use serde;
use serde_json;
use dirs;
use std::io::{BufRead, BufReader};
use std::process::{Stdio};

#[derive(Default)]
struct DownloadState {
    is_downloading: bool,
}

#[derive(serde::Serialize)]
struct FormatInfo {
    name: String,
    size: u64,
    quality: Option<String>,
}

#[derive(serde::Serialize)]
struct VideoInfo {
    title: String,
    formats: Vec<FormatInfo>,
}

#[derive(serde::Serialize, Clone)]
struct DownloadProgress {
    message: String,
}

#[command]
async fn check_you_get_installed() -> Result<bool, String> {
    let output = Command::new("you-get")
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;

    Ok(output.status.success())
}

#[command]
async fn install_you_get() -> Result<(), String> {
    let output = Command::new("pip")
        .arg("install")
        .arg("you-get")
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("Failed to install you-get".to_string());
    }

    Ok(())
}

// 解析进度信息
fn parse_progress(line: &str) -> Option<DownloadProgress> {
    if line.contains("Downloading") {
        Some(DownloadProgress {
            message: line.to_string(),
        })
    } else {
        None
    }
}

#[command]
async fn download_video(
    url: String,
    format: String,
    output_path: Option<String>,
    cookies_path: Option<String>,
    no_caption: bool,
    state: State<'_, Arc<Mutex<DownloadState>>>,
    window: tauri::Window,
) -> Result<(), String> {
    let mut download_state = state.lock().map_err(|e| e.to_string())?;
    if download_state.is_downloading {
        return Err("Another download is in progress".to_string());
    }

    download_state.is_downloading = true;

    let mut command = Command::new("you-get");
    command.arg("--debug");
    
    // 检查是否是 B 站链接
    if url.contains("bilibili.com") {
        command.arg("--format").arg(format);
        if no_caption {
            command.arg("--no-caption");  // 如果选择不下载字幕，添加此参数
        }
    } else {
        // 对于其他网站使用通用格式
        command.arg("--format").arg(format);
    }

    if let Some(path) = output_path {
        if !path.is_empty() {
            command.arg("-o").arg(path);
        }
    }

    if let Some(cookies) = cookies_path {
        if !cookies.is_empty() {
            command.arg("--cookies").arg(cookies);
        }
    }

    command.arg(&url);
    
    // 设置命令的标准输出和错误输出
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    
    // 读取标准输出
    if let Some(stdout) = child.stdout.take() {
        let stdout_reader = BufReader::new(stdout);
        let window_clone = window.clone();
        std::thread::spawn(move || {
            for line in stdout_reader.lines() {
                if let Ok(line) = line {
                    if let Some(progress) = parse_progress(&line) {
                        let _ = window_clone.emit("download-progress", progress);
                    }
                }
            }
        });
    }
    
    // 读取错误输出
    if let Some(stderr) = child.stderr.take() {
        let stderr_reader = BufReader::new(stderr);
        let window_clone = window.clone();
        std::thread::spawn(move || {
            for line in stderr_reader.lines() {
                if let Ok(line) = line {
                    if let Some(progress) = parse_progress(&line) {
                        let _ = window_clone.emit("download-progress", progress);
                    }
                }
            }
        });
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        download_state.is_downloading = false;
        return Err("下载失败".to_string());
    }

    download_state.is_downloading = false;
    Ok(())
}

// 从 JSON 中获取格式的文件大小
fn get_format_size(stream_info: &serde_json::Value) -> u64 {
    if let Some(size) = stream_info["size"].as_u64() {
        size
    } else if let Some(files) = stream_info["files"].as_array() {
        // 如果是多文件格式，计算所有文件的总大小
        files.iter()
            .filter_map(|file| file["size"].as_u64())
            .sum()
    } else {
        0
    }
}

// 从 JSON 中获取格式的质量信息
fn get_format_quality(stream_info: &serde_json::Value) -> Option<String> {
    if let Some(quality) = stream_info["quality"].as_str() {
        Some(quality.to_string())
    } else if let Some(height) = stream_info["height"].as_i64() {
        Some(format!("{}P", height))
    } else {
        None
    }
}

#[command]
async fn get_video_info(url: String, cookies_path: Option<String>) -> Result<VideoInfo, String> {
    let mut command = Command::new("you-get");
    command.arg("--json").arg(&url);
    
    // 如果提供了 cookies 文件路径，添加 --cookies 参数
    if let Some(cookies) = cookies_path {
        command.arg("--cookies").arg(cookies);
    }
    
    let output = command
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("获取视频信息失败: {}", error));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    
    // 解析 JSON 输出
    let info: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| e.to_string())?;

    let title = info["title"]
        .as_str()
        .unwrap_or("未知标题")
        .to_string();

    let mut formats = Vec::new();
    if let Some(streams) = info["streams"].as_object() {
        for (format, stream_info) in streams {
            let size = get_format_size(stream_info);
            let quality = get_format_quality(stream_info);
            formats.push(FormatInfo {
                name: format.clone(),
                size,
                quality,
            });
        }
    }

    // 按文件大小降序排序
    formats.sort_by(|a, b| b.size.cmp(&a.size));

    Ok(VideoInfo { title, formats })
}

#[command]
async fn get_default_download_dir() -> Result<String, String> {
    if let Some(download_dir) = dirs::download_dir() {
        Ok(download_dir.to_string_lossy().to_string())
    } else {
        // 如果无法获取下载目录，返回用户主目录
        if let Some(home_dir) = dirs::home_dir() {
            Ok(home_dir.to_string_lossy().to_string())
        } else {
            Err("无法获取系统下载目录".to_string())
        }
    }
}

fn main() {
    let download_state = Arc::new(Mutex::new(DownloadState::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(download_state)
        .invoke_handler(tauri::generate_handler![
            check_you_get_installed,
            install_you_get,
            download_video,
            get_video_info,
            get_default_download_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
