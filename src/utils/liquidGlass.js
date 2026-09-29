// 液态玻璃的折射层。算法照 kube.io《Liquid Glass in the Browser: Refraction with CSS and SVG》：
// 把玻璃截面当成凸 squircle（Apple 用的就是这种软过渡），按斯涅尔定律算出「离边缘多远 → 光线偏折多少」，
// 把偏折方向编码进位移图的 R/G 通道，交给 SVG feDisplacementMap 去弯折 backdrop。
// 效果是边缘一圈背景被透镜式放大拉弯，中间保持通透——这是 Liquid Glass 和普通磨砂玻璃的根本区别。
//
// 只有 Chromium 支持把 SVG 滤镜用在 backdrop-filter 上（Windows 的 WebView2 属于这一类）；
// macOS / Linux 的 WebKit 不支持，那里保持 CSS 里的模糊玻璃回退，不会坏。

const SVG_NS = "http://www.w3.org/2000/svg";
const REFRACTIVE_INDEX = 1.5;
const PROFILE_SAMPLES = 128;

export const supportsLiquidRefraction = typeof navigator !== "undefined"
  && (navigator.userAgentData?.brands ?? []).some(({ brand }) => brand === "Chromium");

let refractionProfile = null;
let filterHost = null;

// 截面 y = ⁴√(1 - (1-x)⁴)，x 从边缘 0 走到斜面内沿 1
function surfaceHeight(x) {
  return (1 - (1 - x) ** 4) ** 0.25;
}

function surfaceSlope(x) {
  const t = 1 - x;
  return t ** 3 / (1 - t ** 4) ** 0.75;
}

// 斜面上每一处的横向偏移只和「离边缘的距离」有关，先算一条归一化的 1D 曲线，逐像素查表
function getRefractionProfile() {
  if (refractionProfile) return refractionProfile;
  const samples = Array.from({ length: PROFILE_SAMPLES }, (_, index) => {
    const x = (index + 0.5) / PROFILE_SAMPLES;
    const incidence = Math.atan(surfaceSlope(x));
    const refracted = Math.asin(Math.sin(incidence) / REFRACTIVE_INDEX);
    return Math.tan(incidence - refracted) * surfaceHeight(x);
  });
  const maximum = Math.max(...samples);
  refractionProfile = samples.map((value) => value / maximum);
  return refractionProfile;
}

// 圆角矩形的 SDF：返回离边缘的内侧距离和指向外侧的法线
function roundedRectEdge(px, py, halfWidth, halfHeight, radius) {
  const qx = Math.abs(px) - (halfWidth - radius);
  const qy = Math.abs(py) - (halfHeight - radius);
  const ox = Math.max(qx, 0);
  const oy = Math.max(qy, 0);
  const cornerDistance = Math.hypot(ox, oy);
  const depth = radius - cornerDistance - Math.min(Math.max(qx, qy), 0);

  let nx = 0;
  let ny = 0;
  if (ox > 0 && oy > 0) {
    nx = ox / cornerDistance;
    ny = oy / cornerDistance;
  } else if (qx > qy) {
    nx = 1;
  } else {
    ny = 1;
  }
  return { depth, nx: nx * (Math.sign(px) || 1), ny: ny * (Math.sign(py) || 1) };
}

export function createDisplacementMap({ width, height, radius, bezel }) {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  const image = context.createImageData(width, height);
  const profile = getRefractionProfile();
  const halfWidth = width / 2;
  const halfHeight = height / 2;

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const { depth, nx, ny } = roundedRectEdge(x + 0.5 - halfWidth, y + 0.5 - halfHeight, halfWidth, halfHeight, radius);
      let dx = 0;
      let dy = 0;
      if (depth > 0 && depth < bezel) {
        const magnitude = profile[Math.min(PROFILE_SAMPLES - 1, Math.floor((depth / bezel) * PROFILE_SAMPLES))];
        // 往内侧取样：边缘像素显示的是更靠里的背景，看起来就是凸透镜把边缘放大了
        dx = -nx * magnitude;
        dy = -ny * magnitude;
      }
      const offset = (y * width + x) * 4;
      image.data[offset] = 128 + dx * 127;
      image.data[offset + 1] = 128 + dy * 127;
      image.data[offset + 2] = 128;
      image.data[offset + 3] = 255;
    }
  }

  context.putImageData(image, 0, 0);
  return canvas.toDataURL("image/png");
}

// 滤镜宿主不能 display:none——那样 Chromium 会当滤镜不存在
function ensureFilterHost() {
  if (filterHost?.isConnected) return filterHost;
  filterHost = document.createElementNS(SVG_NS, "svg");
  filterHost.setAttribute("aria-hidden", "true");
  filterHost.setAttribute("width", "0");
  filterHost.setAttribute("height", "0");
  filterHost.style.cssText = "position:absolute;width:0;height:0;overflow:hidden;pointer-events:none";
  document.body.append(filterHost);
  return filterHost;
}

function svgElement(name, attributes) {
  const element = document.createElementNS(SVG_NS, name);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, String(value));
  return element;
}

// 先轻模糊再折射：模糊放在折射前，边缘的透镜弯折才保持清晰；饱和度拉高是 Apple 材质「颜色透上来」的那股鲜
export function upsertLiquidGlassFilter(id, { width, height, mapUrl, scale, blur, saturation }) {
  const host = ensureFilterHost();
  host.querySelector(`#${id}`)?.remove();

  const filter = svgElement("filter", {
    id,
    x: 0,
    y: 0,
    width,
    height,
    filterUnits: "userSpaceOnUse",
    // 位移图必须按原值读取；默认的 linearRGB 会把 128 换算成别的值，整块背景都会偏移
    "color-interpolation-filters": "sRGB",
  });
  filter.append(
    svgElement("feGaussianBlur", { in: "SourceGraphic", stdDeviation: blur, result: "blurred" }),
    svgElement("feImage", {
      href: mapUrl, x: 0, y: 0, width, height, preserveAspectRatio: "none", result: "map",
    }),
    svgElement("feDisplacementMap", {
      in: "blurred", in2: "map", scale, xChannelSelector: "R", yChannelSelector: "G", result: "refracted",
    }),
    svgElement("feColorMatrix", { in: "refracted", type: "saturate", values: saturation }),
  );
  host.append(filter);
}

export function removeLiquidGlassFilter(id) {
  filterHost?.querySelector(`#${id}`)?.remove();
}
