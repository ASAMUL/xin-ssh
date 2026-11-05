// 壁纸压不压得住字：量一张壁纸「最暗那一片」和「最亮那一片」有多亮，再按当前明暗判断要不要帮一把。
//
// 浅色模式的字是深色、默认壁纸近白，透明玻璃（首页工具栏、顶栏胶囊）不铺底色也读得清；
// 换成一张大块深色的图，深色字就直接压在暗处上。深色模式反过来怕亮图。
// 看平均亮度不够：天空很亮、头发很暗的图平均下来不算暗，可工具栏偏偏落在头发上，所以看百分位。
// 判断结果只是 <html> 上一个开关（AppWallpaper 挂），具体怎么帮全在 CSS 里。

// 缩到这么宽再量：够分辨出大块的明暗，又只有两千来个像素
const SAMPLE_WIDTH = 64;
// 最暗 / 最亮的那一成像素，大到能盖住一颗胶囊，又不会被几颗高光、几缕发丝带偏
const SHADOW_PERCENTILE = 0.1;
const HIGHLIGHT_PERCENTILE = 0.9;

// 门槛是按文字色倒推的：浅色正文 #3a3a3c 要在约 0.58 以上的底上才有 3.5:1，
// 深色白字要在约 0.45 以下的底上才有 4:1（都是 sRGB 下的亮度，和 CSS 滤镜压缩亮度用的是同一把尺）
const LIGHT_MODE_MIN_SHADOW = 0.58;
const DARK_MODE_MAX_HIGHLIGHT = 0.45;

// 可见度调低时壁纸会透出画布色（main.css 的 --xin-canvas-rgb），用户把图淡化到够亮 / 够暗就不用再帮
const CANVAS_LUMA = { light: 0.95, dark: 0.07 };

// 同一张图换明暗、拖可见度滑块都会重新判断，量过的结果按地址留着
const toneCache = new Map();

export function measureWallpaperTone(url) {
  if (!toneCache.has(url)) {
    const pending = sampleTone(url);
    // 读失败的不留：下次换回这张图还能再试
    pending.catch(() => toneCache.delete(url));
    toneCache.set(url, pending);
  }

  return toneCache.get(url);
}

export function needsLegibilityAssist(tone, mode, opacity) {
  const canvas = CANVAS_LUMA[mode] ?? CANVAS_LUMA.light;
  const blend = (luma) => opacity * luma + (1 - opacity) * canvas;

  return mode === "dark"
    ? blend(tone.highlight) > DARK_MODE_MAX_HIGHLIGHT
    : blend(tone.shadow) < LIGHT_MODE_MIN_SHADOW;
}

async function sampleTone(url) {
  const image = new Image();
  image.src = url;
  await image.decode();

  const width = SAMPLE_WIDTH;
  const height = Math.max(1, Math.round((SAMPLE_WIDTH * image.naturalHeight) / image.naturalWidth));
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  // 默认的低质量缩放是隔点取样，大图缩到 64px 会漏掉整片暗部或只取到几缕发丝
  context.imageSmoothingQuality = "high";
  context.drawImage(image, 0, 0, width, height);

  const { data } = context.getImageData(0, 0, width, height);
  const lumas = new Float32Array(width * height);
  for (let index = 0; index < lumas.length; index += 1) {
    const offset = index * 4;
    lumas[index] = (0.2126 * data[offset] + 0.7152 * data[offset + 1] + 0.0722 * data[offset + 2]) / 255;
  }
  lumas.sort();

  return {
    shadow: percentile(lumas, SHADOW_PERCENTILE),
    highlight: percentile(lumas, HIGHLIGHT_PERCENTILE),
  };
}

function percentile(sorted, ratio) {
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * ratio))];
}
