import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [isRecording, setIsRecording] = useState(false);
  const [recordingPath, setRecordingPath] = useState<string | null>(null);
  const [hasPermission, setHasPermission] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [audioUrl, setAudioUrl] = useState<string | null>(null);

  // Request microphone permission
  const requestPermissions = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ 
        audio: true 
      });
      stream.getTracks().forEach(track => track.stop()); // Stop the stream after getting permission
      setHasPermission(true);
      setErrorMsg("");
    } catch (error) {
      console.error('Permission denied:', error);
      setErrorMsg("Microphone permission denied. Please enable microphone access in your browser settings.");
      setHasPermission(false);
    }
  };

  // Check if already recording when component mounts
  useEffect(() => {
    const checkRecordingStatus = async () => {
      try {
        const recording = await invoke<boolean>("is_recording");
        setIsRecording(recording);
      } catch (error) {
        console.error("Failed to check recording status:", error);
      }
    };
    
    requestPermissions();
    checkRecordingStatus();
  }, []);

  const startRecording = async () => {
    if (!hasPermission) {
      await requestPermissions();
      return;
    }

    try {
      setErrorMsg("");
      setAudioUrl(null); // Clear previous audio
      const path = await invoke<string>("start_recording");
      setIsRecording(true);
      setRecordingPath(path);
      console.log("Recording started, saving to:", path);
    } catch (error) {
      console.error("Failed to start recording:", error);
      setErrorMsg(`Failed to start recording: ${error}`);
    }
  };

  const stopRecording = async () => {
    try {
      setErrorMsg("");
      const path = await invoke<string>("stop_recording");
      setIsRecording(false);
      setRecordingPath(path);
      
      // Convert the file path to a URL that the browser can access
      try {
        const url = convertFileSrc(path);
        setAudioUrl(url);
        console.log("Recording stopped, saved to:", path);
        console.log("Audio URL:", url);
      } catch (urlError) {
        console.error("Failed to convert file path to URL:", urlError);
        setErrorMsg("Failed to load audio file");
      }
    } catch (error) {
      console.error("Failed to stop recording:", error);
      setErrorMsg(`Failed to stop recording: ${error}`);
    }
  };

  return (
    <main className="container">
      <h1>Audio Recorder</h1>

      <div className="recording-section">
        {!hasPermission && (
          <div className="permission-request">
            <p>Microphone permission is required for recording.</p>
            <button onClick={requestPermissions}>
              Request Microphone Permission
            </button>
          </div>
        )}

        {hasPermission && (
          <div className="recording-controls">
            <button 
              onClick={isRecording ? stopRecording : startRecording}
              className={isRecording ? "stop-button" : "start-button"}
              disabled={!hasPermission}
            >
              {isRecording ? "⏹️ Stop Recording" : "🎤 Start Recording"}
            </button>
            
            {isRecording && (
              <div className="recording-indicator">
                <span className="recording-dot"></span>
                Recording in progress...
              </div>
            )}
          </div>
        )}

        {audioUrl && !isRecording && (
          <div className="audio-player">
            <h3>Recorded Audio</h3>
            <audio 
              controls 
              src={audioUrl}
              onError={(e) => {
                console.error("Audio error:", e);
                setErrorMsg("Failed to load audio file");
              }}
              onLoadStart={() => console.log("Audio loading started")}
              onCanPlay={() => console.log("Audio can play")}
            >
              Your browser does not support the audio element.
            </audio>
            <p>Saved to: {recordingPath}</p>
            <p>Audio URL: {audioUrl}</p>
          </div>
        )}

        {errorMsg && (
          <div className="error-message">
            <p style={{ color: "red" }}>{errorMsg}</p>
          </div>
        )}
      </div>
    </main>
  );
}

export default App;
