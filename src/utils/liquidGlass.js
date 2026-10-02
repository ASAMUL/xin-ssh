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

// 高光的光源：主光从正左上 45° 打下来，右下是穿过玻璃的回光。
// 早先是一整圈宽光带（宽度按斜面比例、回光给到主光的一半），深色下看着像描了一圈银灰色的边框，
// 浅色下又把整块玻璃刷白——都不像玻璃。真实的 Liquid Glass 只在朝光的圆角弧段亮一道细线，直边上几乎没有。
const KEY_LIGHT = { x: -Math.SQRT1_2, y: -Math.SQRT1_2 };
const BACK_LIGHT_WEIGHT = 0.3;
// 锐度越高，光越集中在法线正对光源的那段圆弧上：45° 光下直边只剩弧顶的三分之一
const SPECULAR_SHARPNESS = 3;
// 高光线往里衰减的距离（px，不随斜面变宽）：最外一像素全亮，再往里一像素只剩三成多
const SPECULAR_FALLOFF_PX = 1.1;

function mapCanvas(width, height) {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  return { canvas, context, image: context.createImageData(width, height) };
}

function toDataUrl({ canvas, context, image }) {
  context.putImageData(image, 0, 0);
  return canvas.toDataURL("image/png");
}

// 一次遍历同时产出两张图，都从同一条圆角矩形 SDF 和法线来：
// - 位移图：折射用，交给 SVG feDisplacementMap（只有 Chromium 需要）；
// - 高光图：贴着外沿的一道细光——法线朝向光源的圆角弧段最亮，直边淡出。
//   1px 亮边是整圈一样亮的，这道光有明暗走向，看着才是「光打在玻璃上」；做成普通背景图，所有内核都能用。
function createGlassMaps({ width, height, radius, bezel, displacement }) {
  const displacementMap = displacement ? mapCanvas(width, height) : null;
  const specularMap = mapCanvas(width, height);
  const profile = getRefractionProfile();
  const halfWidth = width / 2;
  const halfHeight = height / 2;

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const { depth, nx, ny } = roundedRectEdge(x + 0.5 - halfWidth, y + 0.5 - halfHeight, halfWidth, halfHeight, radius);
      const offset = (y * width + x) * 4;
      const onBezel = depth > 0 && depth < bezel;

      if (displacementMap) {
        let dx = 0;
        let dy = 0;
        if (onBezel) {
          const magnitude = profile[Math.min(PROFILE_SAMPLES - 1, Math.floor((depth / bezel) * PROFILE_SAMPLES))];
          // 往内侧取样：边缘像素显示的是更靠里的背景，看起来就是凸透镜把边缘放大了
          dx = -nx * magnitude;
          dy = -ny * magnitude;
        }
        const data = displacementMap.image.data;
        data[offset] = 128 + dx * 127;
        data[offset + 1] = 128 + dy * 127;
        data[offset + 2] = 128;
        data[offset + 3] = 255;
      }

      if (onBezel) {
        const facing = nx * KEY_LIGHT.x + ny * KEY_LIGHT.y;
        const light = Math.max(0, facing) ** SPECULAR_SHARPNESS
          + BACK_LIGHT_WEIGHT * Math.max(0, -facing) ** SPECULAR_SHARPNESS;
        const intensity = Math.min(1, light * Math.exp(-Math.max(0, depth - 0.5) / SPECULAR_FALLOFF_PX));
        const data = specularMap.image.data;
        data[offset] = 255;
        data[offset + 1] = 255;
        data[offset + 2] = 255;
        data[offset + 3] = Math.round(intensity * 255);
      }
    }
  }

  return {
    displacementUrl: displacementMap ? toDataUrl(displacementMap) : null,
    specularUrl: toDataUrl(specularMap),
  };
}

// 贴图只和几何有关：同样尺寸的玻璃（同一排胶囊、每次弹出的同一个菜单、窗口来回缩放）
// 不必每次重画像素、重编码 PNG。Map 按插入顺序迭代，命中时删了重插就是「最近用过」，
// 超出上限从最旧的一头淘汰；一张图几 KB 到几十 KB，48 张封顶。
const MAP_CACHE_LIMIT = 48;
const mapCache = new Map();

export function getGlassMaps({ width, height, radius, bezel, displacement = true }) {
  const key = [width, height, radius, bezel, displacement ? 1 : 0].join(":");
  const cached = mapCache.get(key);
  if (cached) {
    mapCache.delete(key);
    mapCache.set(key, cached);
    return cached;
  }

  const maps = createGlassMaps({ width, height, radius, bezel, displacement });
  mapCache.set(key, maps);
  if (mapCache.size > MAP_CACHE_LIMIT) mapCache.delete(mapCache.keys().next().value);
  return maps;
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

const CHANNEL_ONLY = {
  R: "1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 0 1",
  G: "0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 0 1",
  B: "0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 0 1",
};

// 色散：真实玻璃对蓝光折得比红光狠，边缘会析出一道极淡的彩边。R / G / B 各按不同强度弯一次再拼回去。
// 三个通道先各自压成不透明（colormatrix 在非预乘空间里算，颜色不会被 alpha 压暗）再相加，
// 最后用中间那次的 alpha 裁回去——窗口是透明的（Tauri 里底下是 Mica），直接相加会把半透明的地方加成不透明的暗块。
function dispersedDisplacement({ scale, dispersion }) {
  const pass = (channel, factor) => [
    svgElement("feDisplacementMap", {
      in: "blurred", in2: "map", scale: scale * factor, xChannelSelector: "R", yChannelSelector: "G", result: `shift${channel}`,
    }),
    svgElement("feColorMatrix", { in: `shift${channel}`, type: "matrix", values: CHANNEL_ONLY[channel], result: `only${channel}` }),
  ];
  return [
    ...pass("R", 1 - dispersion),
    ...pass("G", 1),
    ...pass("B", 1 + dispersion),
    svgElement("feComposite", { in: "onlyR", in2: "onlyG", operator: "arithmetic", k2: 1, k3: 1, result: "rg" }),
    svgElement("feComposite", { in: "rg", in2: "onlyB", operator: "arithmetic", k2: 1, k3: 1, result: "rgb" }),
    svgElement("feComposite", { in: "rgb", in2: "shiftG", operator: "in", result: "refracted" }),
  ];
}

// 先轻模糊再折射：模糊放在折射前，边缘的透镜弯折才保持清晰；饱和度拉高是 Apple 材质「颜色透上来」的那股鲜
export function upsertLiquidGlassFilter(id, { width, height, mapUrl, scale, blur, saturation, dispersion = 0 }) {
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
    ...(dispersion > 0
      ? dispersedDisplacement({ scale, dispersion })
      : [svgElement("feDisplacementMap", {
        in: "blurred", in2: "map", scale, xChannelSelector: "R", yChannelSelector: "G", result: "refracted",
      })]),
    svgElement("feColorMatrix", { in: "refracted", type: "saturate", values: saturation }),
  );
  host.append(filter);
}

export function removeLiquidGlassFilter(id) {
  filterHost?.querySelector(`#${id}`)?.remove();
}
