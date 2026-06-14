// Load query param "url" into url input
const params = new URLSearchParams(window.location.search);
const url = params.get("url");
if (url) {
  document.getElementById("urlInput").value = url;
}

document.getElementById("downloadForm").addEventListener("submit", (e) => {
  e.preventDefault();
  onDownload();
});

const wsUri = "/api/ws";

async function onDownload() {
  let websocket = null;
  const downloadUrl = document.getElementById("url-input").value;
  const mediaType = document.getElementById("mediaType").value;
  changeUI({ message: { message_type: "clear_manifest" } });

  function initializeWebSocketListeners(ws) {
    let uuid = uuidv4();

    ws.addEventListener("open", () => {
      console.log("CONNECTED");
      const downloadMessage = JSON.stringify({
        client_id: uuid,
        url: downloadUrl,
        media_type: mediaType,
      });
      ws.send(downloadMessage);
    });

    ws.addEventListener("close", () => {
      console.log("DISCONNECTED");
    });

    ws.addEventListener("message", (e) => {
      console.log(`RECEIVED: ${e.data}`);
      const parsedData = JSON.parse(e.data);
      if (parsedData["message_type"] === "video_ready") {
        blobDownload(parsedData["download_url"], parsedData["video_title"]);
        changeUI({ message: parsedData });
      } else if (parsedData["message_type"] === "request_finished") {
        changeUI({ message: parsedData });
      } else if (parsedData["message_type"] === "error") {
        changeUI({ message: parsedData });
      } else {
        console.error("Unknown message_type:", parsedData["message_type"]);
      }
    });

    ws.addEventListener("error", (e) => {
      console.log(`ERROR`, e);
      alert(`WebSocket error: ${e}`);
    });
  }

  window.addEventListener("pageshow", (event) => {
    if (event.persisted) {
      websocket = new WebSocket(wsUri);
      initializeWebSocketListeners(websocket);
    }
  });

  console.log("OPENING");
  websocket = new WebSocket(wsUri);
  initializeWebSocketListeners(websocket);
}

async function blobDownload(url, filename) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(res.statusText);
  const blob = await res.blob();
  const a = document.createElement("a");
  const objectUrl = URL.createObjectURL(blob);
  a.href = objectUrl;
  a.download = filename || url.split("/").pop() || "download";
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(objectUrl);
}

// UUIDv4 using crypto.getRandomValues — RFC 4122 compliant
function uuidv4() {
  // create 16 random bytes
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);

  // Per RFC 4122: set version to 4 (0100)
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  // Per RFC 4122: set variant to 10xx
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  // convert bytes to hex and format with dashes
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0"));
  return (
    hex.slice(0, 4).join("") +
    "-" +
    hex.slice(4, 6).join("") +
    "-" +
    hex.slice(6, 8).join("") +
    "-" +
    hex.slice(8, 10).join("") +
    "-" +
    hex.slice(10, 16).join("")
  );
}

function changeUI(uiState) {
  const { message } = uiState;
  const messageType = message["message_type"];

  if (messageType === "video_ready") {
    const videoId = message["video_id"];
    const videoTitle = message["video_title"];
    const id = `video-li-${videoId}`;
    let li = document.getElementById(id);
    if (!li) {
      const manifestList = document.getElementById("manifest-list");
      document.getElementById("manifest-container").style.display = "block";

      li = document.createElement("li");
      li.id = id;
      li.textContent = videoTitle;

      manifestList.appendChild(li);
    }
  } else if (messageType === "request_finished") {
    const success = message["success"];
    const statusElement = success ? "done-message" : "error-message";
    document.getElementById(statusElement).style.display = "block";
  } else if (messageType === "clear_manifest") {
    const manifestList = document.getElementById("manifest-list");
    manifestList.replaceChildren();

    document.getElementById("manifest-container").style.display = "none";
    document.getElementById("done-message").style.display = "none";
    document.getElementById("error-message").style.display = "none";
  } else if (messageType === "error") {
    const errorMessage =
      message["error_message"] || "An unknown error occurred";
    alert(`Error: ${errorMessage}`);
  }
}
