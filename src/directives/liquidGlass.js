import {
  getGlassMaps,
  removeLiquidGlassFilter,
  supportsLiquidRefraction,
  upsertLiquidGlassFilter,
} from "../utils/liquidGlass";

// v-liquid-glass 负责「按尺寸生成」的两层：
// - 高光线：贴着外沿、只在朝光的圆角弧段亮的一道细光，写进 --lg-specular，由 .liquid-glass::after 画出来（所有内核都有）；
// - 折射：位移图 + backdrop-filter（只有 Chromium / WebView2）。
// 材质本身（通透底色、描边、投影）由 .liquid-glass 类提供——类写在模板里，
// 指令不去增删 class，组件重渲染改 class 时不会把材质冲掉。
const DEFAULTS = {
  // 斜面宽度（px）；默认取圆角的 0.8，胶囊就是整条边都在折射
  bezel: null,
  // 最大偏折像素：越大边缘弯得越狠。胶囊控件 14~20 比较像 iOS
  scale: 18,
  blur: 2.5,
  saturation: 1.5,
  // 色散强度：R / B 通道比 G 少弯 / 多弯的比例，0 关闭。
  // 曾给到 0.07，压在樱色水滴、终端彩字上时边缘析出的彩边太明显，会被当成玻璃描了一圈颜色
  dispersion: 0.03,
};

// 色散要把位移做三遍，成本随面积涨；整条顶栏、大对话框底下本来也没有高对比的边缘可析出彩边，只给控件尺寸的玻璃开
const DISPERSION_MAX_AREA = 160_000;

let sequence = 0;

// 所有玻璃共用一个 ResizeObserver，尺寸直接取回调给的 borderBoxSize。
// 以前是每块玻璃在 requestAnimationFrame 里自己读 offsetWidth：页面刚切换、布局还脏着的时候，
// 这一读会逼浏览器当场把整页重排一遍，几块玻璃读写交错还会反复重排（控制台里的 Forced reflow）。
// ResizeObserver 回调在浏览器自己排完版之后才调用，这时读尺寸、读计算样式都不会再触发重排；
// 先把这一轮所有玻璃都量完，再统一生成贴图、写样式。
// borderBoxSize 和 offsetWidth 一样不受 transform 影响，按下时的缩放不会让贴图重算。
const surfaces = new Map();
const resizeObserver = new ResizeObserver((entries) => {
  const measured = [];
  for (const entry of entries) {
    const surface = surfaces.get(entry.target);
    const box = entry.borderBoxSize?.[0];
    if (!surface || !box) continue;
    const width = Math.round(box.inlineSize);
    const height = Math.round(box.blockSize);
    // 被隐藏（display: none）时尺寸是 0：留着上一次的贴图，重新显示且尺寸没变时什么都不用做
    if (!width || !height) continue;
    const radius = parseFloat(getComputedStyle(entry.target).borderTopLeftRadius) || 0;
    measured.push([surface, { width, height, radius }]);
  }
  for (const [surface, geometry] of measured) surface.render(geometry);
});

class LiquidGlassSurface {
  constructor(element, options) {
    this.element = element;
    this.id = `xin-liquid-glass-${++sequence}`;
    this.signature = "";
    this.geometry = null;
    this.options = { ...DEFAULTS, ...options };
    surfaces.set(element, this);
    resizeObserver.observe(element);
  }

  // 模板每次重渲染都会传进一个新的选项对象，值没变就不用重算
  update(options) {
    const next = { ...DEFAULTS, ...options };
    if (Object.keys(next).every((key) => next[key] === this.options[key])) return;
    this.options = next;
    if (this.geometry) this.render(this.geometry);
  }

  render(geometry) {
    this.geometry = geometry;
    const { width, height } = geometry;
    const radius = Math.min(geometry.radius, width / 2, height / 2);
    const { scale, blur, saturation } = this.options;
    const dispersion = width * height <= DISPERSION_MAX_AREA ? this.options.dispersion : 0;
    const bezel = Math.max(2, Math.min(this.options.bezel ?? Math.max(radius * 0.8, 8), width / 2, height / 2));
    const signature = [width, height, radius, bezel, scale, blur, saturation, dispersion].join(":");
    if (signature !== this.signature) {
      this.signature = signature;
      const { displacementUrl, specularUrl } = getGlassMaps({
        width, height, radius, bezel, displacement: supportsLiquidRefraction,
      });
      this.element.style.setProperty("--lg-specular", `url("${specularUrl}")`);
      if (supportsLiquidRefraction) {
        upsertLiquidGlassFilter(this.id, {
          width, height, scale, blur, saturation, dispersion, mapUrl: displacementUrl,
        });
      }
    }
    // 内联样式压过 .liquid-glass 里的模糊回退
    if (supportsLiquidRefraction) this.element.style.setProperty("backdrop-filter", `url(#${this.id})`);
  }

  destroy() {
    resizeObserver.unobserve(this.element);
    surfaces.delete(this.element);
    removeLiquidGlassFilter(this.id);
    this.element.style.removeProperty("backdrop-filter");
    this.element.style.removeProperty("--lg-specular");
  }
}

export const vLiquidGlass = {
  mounted(element, { value }) {
    element.liquidGlassSurface = new LiquidGlassSurface(element, value);
  },
  updated(element, { value }) {
    element.liquidGlassSurface?.update(value);
  },
  unmounted(element) {
    element.liquidGlassSurface?.destroy();
    delete element.liquidGlassSurface;
  },
};

// 弹出层（菜单、弹层、Toast、对话框）由 Nuxt UI 经 portal 渲染到 body，模板里挂不了 v-liquid-glass；
// 这里盯着 DOM，带玻璃类的元素一出现就补上高光带和折射层，移除时一并回收滤镜。
// 只看新增的元素节点、判断一次类名，终端/编辑器频繁改 DOM 时开销可以忽略。
export function autoAttachLiquidGlass(rules) {
  const surfaces = new Map();
  const selector = rules.map((rule) => rule.selector).join(",");

  function attach(element) {
    if (surfaces.has(element) || element.liquidGlassSurface) return;
    const rule = rules.find(({ selector }) => element.matches(selector));
    if (rule) surfaces.set(element, new LiquidGlassSurface(element, rule.options));
  }

  function scan(node) {
    if (!(node instanceof Element)) return;
    if (node.matches(selector)) attach(node);
    node.querySelectorAll(selector).forEach(attach);
  }

  const observer = new MutationObserver((records) => {
    for (const record of records) record.addedNodes.forEach(scan);
    for (const [element, surface] of surfaces) {
      if (element.isConnected) continue;
      surface.destroy();
      surfaces.delete(element);
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
  scan(document.body);

  return () => {
    observer.disconnect();
    surfaces.forEach((surface) => surface.destroy());
    surfaces.clear();
  };
}

// 鼠标当光源：iOS 靠陀螺仪让玻璃上的光跟着手机转，桌面没有陀螺仪，就让一团柔光跟着指针走。
// 整个文档只挂一个 pointermove、每帧最多写一次坐标，只写进指针正下方那块玻璃；
// 显不显示、多亮由 CSS 决定（liquid-glass.css 的 --lg-glow-*），这里不碰样式。
// 玻璃的位置只在指针刚移进来时量一次：点「连接 | 工作区」这类按钮时指针正停在玻璃上，
// 每帧都 getBoundingClientRect 的话，切换页面那一帧会被它逼着提前整页重排。滚动、缩放窗口后作废重量。
export function trackLiquidGlassPointer() {
  let frame = 0;
  let latest = null;
  let hovered = null;

  function paint() {
    frame = 0;
    const glass = latest.target instanceof Element ? latest.target.closest(".liquid-glass") : null;
    if (!glass) {
      hovered = null;
      return;
    }
    if (hovered?.glass !== glass) hovered = { glass, rect: glass.getBoundingClientRect() };
    const { rect } = hovered;
    glass.style.setProperty("--lg-pointer-x", `${latest.clientX - rect.left}px`);
    glass.style.setProperty("--lg-pointer-y", `${latest.clientY - rect.top}px`);
  }

  function handlePointerMove(event) {
    latest = event;
    if (!frame) frame = requestAnimationFrame(paint);
  }

  function forgetPosition() {
    hovered = null;
  }

  document.addEventListener("pointermove", handlePointerMove, { passive: true });
  window.addEventListener("scroll", forgetPosition, { capture: true, passive: true });
  window.addEventListener("resize", forgetPosition, { passive: true });
  return () => {
    cancelAnimationFrame(frame);
    document.removeEventListener("pointermove", handlePointerMove);
    window.removeEventListener("scroll", forgetPosition, { capture: true });
    window.removeEventListener("resize", forgetPosition);
  };
}

const reducedMotion = typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

// 选中水滴换位置时被拉长、落定时回弹一下——iOS 26 分段控件里那颗水滴「流过去」的手感。
// 挂在 UTabs 上、值传当前选中项：v-liquid-stretch="activeView"。
// 平移本身仍是 Nuxt UI / Reka 的 translate + width 过渡，这里只叠一段 scale 动画（独立属性，不和 translate 打架），
// 用 Web Animations 播，不往元素上加类——组件重渲染时不会把动画冲掉。
const STRETCH_KEYFRAMES = [
  { scale: "1 1" },
  { scale: "1.14 0.88", offset: 0.32 },
  { scale: "0.97 1.03", offset: 0.68 },
  { scale: "1 1" },
];

export const vLiquidStretch = {
  updated(element, { value, oldValue }) {
    if (value === oldValue || reducedMotion?.matches) return;
    element.querySelector('[data-slot="indicator"]')?.animate(STRETCH_KEYFRAMES, {
      duration: 460,
      easing: "ease-out",
    });
  },
};
