// Ctrl+V 粘贴到远程目录：给截图、文字起默认文件名，把剪贴板像素编码成 PNG

const PUBLIC_KEY_PATTERN = /^(ssh-(rsa|ed25519|dss)|ecdsa-sha2-|sk-(ssh|ecdsa)-)/;

export function defaultPasteFileName(kind, text = "") {
  const stamp = timestampForFileName(new Date());
  if (kind === "image") return `screenshot_${stamp}.png`;
  return `clipboard_${stamp}${textExtension(text)}`;
}

// 粘贴最多的是配置片段和公钥，猜对扩展名，编辑器打开时才有语法高亮
function textExtension(text) {
  const trimmed = text.trim();
  if (PUBLIC_KEY_PATTERN.test(trimmed)) return ".pub";
  if (/^[[{]/.test(trimmed)) {
    try {
      JSON.parse(trimmed);
      return ".json";
    } catch {
      // 只是长得像 JSON
    }
  }
  return ".txt";
}

function timestampForFileName(date) {
  const pad = (value) => String(value).padStart(2, "0");
  const day = `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}`;
  return `${day}_${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

// 后端给的是「宽、高（各 4 字节小端）+ RGBA 像素」，交给 canvas 编码成 PNG
export async function decodeClipboardImage(buffer) {
  const view = new DataView(buffer);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  const pixels = new Uint8ClampedArray(buffer, 8, width * height * 4);

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  canvas.getContext("2d").putImageData(new ImageData(pixels, width, height), 0, 0);

  const blob = await new Promise((resolve, reject) => {
    canvas.toBlob((result) => (result ? resolve(result) : reject(new Error("PNG encoding failed"))), "image/png");
  });
  return { blob, width, height };
}

export function countLines(text) {
  if (!text) return 0;
  let lines = 1;
  for (let index = text.indexOf("\n"); index !== -1; index = text.indexOf("\n", index + 1)) lines += 1;
  return text.endsWith("\n") ? lines - 1 : lines;
}
