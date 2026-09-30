import {
  createGlassMaps,
  removeLiquidGlassFilter,
  supportsLiquidRefraction,
  upsertLiquidGlassFilter,
} from "../utils/liquidGlass";

// v-liquid-glass 负责「按尺寸生成」的两层：
// - 高光带：沿斜面的一圈柔光，写进 --lg-specular，由 .liquid-glass::after 画出来（所有内核都有）；
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
  // 色散强度：R / B 通道比 G 少弯 / 多弯的比例，0 关闭
  dispersion: 0.07,
};

// 色散要把位移做三遍，成本随面积涨；整条顶栏、大对话框底下本来也没有高对比的边缘可析出彩边，只给控件尺寸的玻璃开
const DISPERSION_MAX_AREA = 160_000;

let sequence = 0;

class LiquidGlassSurface {
  constructor(element, options) {
    this.element = element;
    this.id = `xin-liquid-glass-${++sequence}`;
    this.frame = 0;
    this.signature = "";
    this.options = { ...DEFAULTS, ...options };
    this.observer = new ResizeObserver(() => this.schedule());
    this.observer.observe(element);
    this.schedule();
  }

  update(options) {
    this.options = { ...DEFAULTS, ...options };
    this.schedule();
  }

  // 尺寸在过渡动画里会连续变化，合并到每帧最多重算一次
  schedule() {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.render();
    });
  }

  render() {
    // offsetWidth 不受按下缩放等 transform 影响，位移图按布局尺寸生成才不会每次点击都重算
    const width = this.element.offsetWidth;
    const height = this.element.offsetHeight;
    if (!width || !height) return;

    const radius = Math.min(parseFloat(getComputedStyle(this.element).borderTopLeftRadius) || 0, width / 2, height / 2);
    const { scale, blur, saturation } = this.options;
    const dispersion = width * height <= DISPERSION_MAX_AREA ? this.options.dispersion : 0;
    const bezel = Math.max(2, Math.min(this.options.bezel ?? Math.max(radius * 0.8, 8), width / 2, height / 2));
    const signature = [width, height, radius, bezel, scale, blur, saturation, dispersion].join(":");
    if (signature !== this.signature) {
      this.signature = signature;
      const { displacementUrl, specularUrl } = createGlassMaps({
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
    cancelAnimationFrame(this.frame);
    this.observer.disconnect();
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
export function trackLiquidGlassPointer() {
  let frame = 0;
  let latest = null;

  function paint() {
    frame = 0;
    const glass = latest.target instanceof Element ? latest.target.closest(".liquid-glass") : null;
    if (!glass) return;
    const rect = glass.getBoundingClientRect();
    glass.style.setProperty("--lg-pointer-x", `${latest.clientX - rect.left}px`);
    glass.style.setProperty("--lg-pointer-y", `${latest.clientY - rect.top}px`);
  }

  function handlePointerMove(event) {
    latest = event;
    if (!frame) frame = requestAnimationFrame(paint);
  }

  document.addEventListener("pointermove", handlePointerMove, { passive: true });
  return () => {
    cancelAnimationFrame(frame);
    document.removeEventListener("pointermove", handlePointerMove);
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
