import {
  createDisplacementMap,
  removeLiquidGlassFilter,
  supportsLiquidRefraction,
  upsertLiquidGlassFilter,
} from "../utils/liquidGlass";

// v-liquid-glass 只负责「折射」这一层：量尺寸、生成位移图、挂 backdrop-filter。
// 材质本身（通透底色、高光描边、樱色焦散）由 .liquid-glass 类提供——类写在模板里，
// 指令不去增删 class，组件重渲染改 class 时不会把材质冲掉。
const DEFAULTS = {
  // 斜面宽度（px）；默认取圆角的 0.8，胶囊就是整条边都在折射
  bezel: null,
  // 最大偏折像素：越大边缘弯得越狠。胶囊控件 14~20 比较像 iOS
  scale: 18,
  blur: 2.5,
  saturation: 1.8,
};

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
    const bezel = Math.max(2, Math.min(this.options.bezel ?? Math.max(radius * 0.8, 8), width / 2, height / 2));
    const signature = [width, height, radius, bezel, scale, blur, saturation].join(":");
    if (signature !== this.signature) {
      this.signature = signature;
      upsertLiquidGlassFilter(this.id, {
        width,
        height,
        scale,
        blur,
        saturation,
        mapUrl: createDisplacementMap({ width, height, radius, bezel }),
      });
    }
    // 内联样式压过 .liquid-glass 里的模糊回退
    this.element.style.setProperty("backdrop-filter", `url(#${this.id})`);
  }

  destroy() {
    cancelAnimationFrame(this.frame);
    this.observer.disconnect();
    removeLiquidGlassFilter(this.id);
    this.element.style.removeProperty("backdrop-filter");
  }
}

export const vLiquidGlass = {
  mounted(element, { value }) {
    if (!supportsLiquidRefraction) return;
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
// 这里盯着 DOM，带玻璃类的元素一出现就补上折射层，移除时一并回收滤镜。
// 只看新增的元素节点、判断一次类名，终端/编辑器频繁改 DOM 时开销可以忽略。
export function autoAttachLiquidGlass(rules) {
  if (!supportsLiquidRefraction) return () => {};
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
