use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use tauri::{State, Manager};

// State to manage recording
pub struct RecordingState {
    pub is_recording: Arc<Mutex<bool>>,
    pub output_path: Arc<Mutex<Option<String>>>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            output_path: Arc::new(Mutex::new(None)),
        }
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn start_recording(app_handle: tauri::AppHandle, state: State<'_, RecordingState>) -> Result<String, String> {
    let mut is_recording = state.is_recording.lock().map_err(|e| e.to_string())?;
    
    if *is_recording {
        return Err("Already recording".to_string());
    }
    
    *is_recording = true;
    
    // Get the app data directory using Tauri 2.0 API
    let app_data_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    // Create the directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
        return Err(format!("Failed to create app data directory: {}", e));
    }
    
    let output_path = app_data_dir.join("recording.wav");
    let output_path_str = output_path.to_string_lossy().to_string();
    
    *state.output_path.lock().map_err(|e| e.to_string())? = Some(output_path_str.clone());
    
    let is_recording_clone = state.is_recording.clone();
    let output_path_clone = output_path.clone();
    
    // Start recording in a separate thread
    thread::spawn(move || {
        if let Err(e) = record_audio(is_recording_clone, output_path_clone) {
            eprintln!("Recording error: {}", e);
        }
    });
    
    Ok(output_path_str)
}

#[tauri::command]
async fn stop_recording(state: State<'_, RecordingState>) -> Result<String, String> {
    let mut is_recording = state.is_recording.lock().map_err(|e| e.to_string())?;
    
    if !*is_recording {
        return Err("Not recording".to_string());
    }
    
    *is_recording = false;
    
    // Wait a bit for the recording to stop
    thread::sleep(Duration::from_millis(100));
    
    let output_path = state.output_path.lock().map_err(|e| e.to_string())?;
    match output_path.as_ref() {
        Some(path) => Ok(path.clone()),
        None => Err("No recording path found".to_string()),
    }
}

#[tauri::command]
async fn is_recording(state: State<'_, RecordingState>) -> Result<bool, String> {
    let is_recording = state.is_recording.lock().map_err(|e| e.to_string())?;
    Ok(*is_recording)
}

fn record_audio(is_recording: Arc<Mutex<bool>>, output_path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device available")?;
    
    let config = device.default_input_config()?;
    let sample_format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    
    let spec = WavSpec {
        channels: config.channels as _,
        sample_rate: config.sample_rate.0 as _,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let writer = WavWriter::create(output_path, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));
    
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let writer_ref = writer.clone();
            let is_recording_ref = is_recording.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let recording = is_recording_ref.lock().unwrap();
                    if !*recording {
                        return;
                    }
                    
                    if let Ok(mut writer_guard) = writer_ref.lock() {
                        if let Some(ref mut writer) = writer_guard.as_mut() {
                            for &sample in data {
                                let sample = (sample * i16::MAX as f32) as i16;
                                let _ = writer.write_sample(sample);
                            }
                        }
                    }
                },
                |err| eprintln!("Stream error: {}", err),
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let writer_ref = writer.clone();
            let is_recording_ref = is_recording.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let recording = is_recording_ref.lock().unwrap();
                    if !*recording {
                        return;
                    }
                    
                    if let Ok(mut writer_guard) = writer_ref.lock() {
                        if let Some(ref mut writer) = writer_guard.as_mut() {
                            for &sample in data {
                                let _ = writer.write_sample(sample);
                            }
                        }
                    }
                },
                |err| eprintln!("Stream error: {}", err),
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let writer_ref = writer.clone();
            let is_recording_ref = is_recording.clone();
            device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let recording = is_recording_ref.lock().unwrap();
                    if !*recording {
                        return;
                    }
                    
                    if let Ok(mut writer_guard) = writer_ref.lock() {
                        if let Some(ref mut writer) = writer_guard.as_mut() {
                            for &sample in data {
                                let sample = (sample as i32 - 32768) as i16;
                                let _ = writer.write_sample(sample);
                            }
                        }
                    }
                },
                |err| eprintln!("Stream error: {}", err),
                None,
            )?
        }
        _ => return Err("Unsupported sample format".into()),
    };
    
    stream.play()?;
    
    // Keep the stream alive while recording
    loop {
        thread::sleep(Duration::from_millis(100));
        let recording = is_recording.lock().unwrap();
        if !*recording {
            break;
        }
    }
    
    // Finalize the WAV file
    if let Ok(mut writer_guard) = writer.lock() {
        if let Some(writer) = writer_guard.take() {
            let _ = writer.finalize();
        }
    }
    
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(RecordingState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            start_recording,
            stop_recording,
            is_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
