import { onBeforeUnmount, ref } from "vue";
import { setLiquidGlassStrength } from "../utils/liquidGlass";

// 液态玻璃放大镜的动作引擎（iOS 26 标签栏按住选中项时浮起的那颗透镜，给 LiquidLensTabs 用）：
// 从某个格子原地、原大鼓起（弯折从 0 长出来），跟着手指走，松手滑进目标格子、停稳，
// 再缩回那个格子的大小、弯折退回 0。只管运动和逐帧写样式；透镜元素由组件自己画（挂 v-liquid-glass 的 magnify），
// 格子的位置由组件量好传进来，统一是相对容器的 { left, top, width, height, centerX, centerY }。

// 弹簧（频率 Hz、阻尼比）：从静止起步、先加速再减速，不像指数逼近那样第一帧就最快，点一下不会「跳」。
// 拖动时跟手要紧；松手后滑进格子留一点点过冲，像水滴落定
const DRAG_SPRING = { frequency: 7, damping: 0.85 };
const SETTLE_SPRING = { frequency: 4.5, damping: 0.72 };
const LIFT_SPRING = { frequency: 5, damping: 0.75 };
const DROP_SPRING = { frequency: 5.5, damping: 1 };
// 点一下也要让透镜完整鼓起来再落下，不然只闪一下
const MIN_LIFT_MS = 180;
// 移动时被拉长的上限，以及多快（px/s）算拉满
const MAX_STRETCH = 0.2;
const STRETCH_SPEED = 1800;

const reducedMotionQuery = typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

export function prefersReducedMotion() {
  return Boolean(reducedMotionQuery?.matches);
}

// 量一个元素相对容器的位置
export function measureBox(element, containerRect) {
  const rect = element.getBoundingClientRect();
  const left = rect.left - containerRect.left;
  const top = rect.top - containerRect.top;
  return {
    left,
    top,
    width: rect.width,
    height: rect.height,
    centerX: left + rect.width / 2,
    centerY: top + rect.height / 2,
  };
}

// 半隐式欧拉积分一步弹簧
function spring(value, velocity, target, { frequency, damping }, seconds) {
  const omega = 2 * Math.PI * frequency;
  const nextVelocity = velocity + (-omega * omega * (value - target) - 2 * damping * omega * velocity) * seconds;
  return [value + nextVelocity * seconds, nextVelocity];
}

function lerp(from, to, amount) {
  return from + (to - from) * amount;
}

/**
 * @param {object} options
 * @param {import("vue").Ref<HTMLElement>} options.container 透镜的定位容器；水滴的淡入淡出写在它的 --droplet-opacity 上
 * @param {import("vue").Ref<HTMLElement>} options.lens 透镜元素
 * @param {import("vue").Ref<HTMLElement>} [options.tint] 只露出透镜那一块的着色层，可以没有
 * @param {() => {width: number, height: number}} options.lensSize 鼓满时的透镜尺寸
 * @param {(x: number) => number} [options.clampX] 透镜中心能到的横向范围
 */
export function useLiquidLens({ container, lens, tint = null, lensSize, clampX = (x) => x }) {
  const lensing = ref(false);
  // 逐帧的状态不进响应式：每帧直接写样式，不让 Vue 跟着重渲染
  const motion = {
    x: 0, vx: 0, targetX: 0, presence: 0, vp: 0,
    holding: false, dropping: false, pressedAt: 0, anchor: null, strength: -1, bounds: null,
  };
  let frame = 0;
  let lastTime = 0;

  // 从 anchor 这一格鼓起，透镜中心朝 x 走；上一颗还没落定又按下，就接着它当前的样子继续
  function press(anchor, x = anchor.centerX) {
    if (!lensing.value) {
      motion.x = anchor.centerX;
      motion.vx = 0;
      motion.presence = 0;
      motion.vp = 0;
    }
    const element = container.value;
    motion.bounds = { width: element.clientWidth, height: element.clientHeight };
    motion.anchor = anchor;
    motion.targetX = clampX(x);
    motion.holding = true;
    motion.dropping = false;
    motion.pressedAt = performance.now();
    lensing.value = true;
    if (!frame) {
      lastTime = performance.now();
      frame = requestAnimationFrame(step);
    }
  }

  function move(x) {
    if (!motion.holding) return;
    motion.targetX = clampX(x);
  }

  // 松手：滑进 target 这一格，停稳后落下
  function release(target) {
    if (!motion.holding) return;
    motion.holding = false;
    motion.anchor = target;
    motion.targetX = target.centerX;
  }

  function advance(seconds, now) {
    [motion.x, motion.vx] = spring(
      motion.x, motion.vx, motion.targetX, motion.holding ? DRAG_SPRING : SETTLE_SPRING, seconds,
    );
    // 落下这一步不可逆：过冲回弹不会让它再鼓起来
    if (!motion.holding && !motion.dropping
      && now - motion.pressedAt >= MIN_LIFT_MS
      && Math.abs(motion.x - motion.targetX) < 1.5
      && Math.abs(motion.vx) < 40) {
      motion.dropping = true;
    }
    [motion.presence, motion.vp] = spring(
      motion.presence, motion.vp, motion.dropping ? 0 : 1,
      motion.dropping ? DROP_SPRING : LIFT_SPRING, seconds,
    );
  }

  function step(now) {
    // 拆成不超过 8ms 的小步积分，掉帧时弹簧也不会炸开
    let remaining = Math.min(now - lastTime, 64) / 1000;
    lastTime = now;
    while (remaining > 0) {
      const slice = Math.min(remaining, 0.008);
      advance(slice, now);
      remaining -= slice;
    }
    paint();

    if (motion.dropping && motion.presence < 0.01) {
      finish();
      return;
    }
    frame = requestAnimationFrame(step);
  }

  function paint() {
    const { width: lensWidth, height: lensHeight } = lensSize();
    const { anchor, bounds } = motion;
    const presence = Math.max(0, motion.presence);
    const lift = Math.min(presence, 1);
    const stretch = 1 + Math.min(Math.abs(motion.vx) / STRETCH_SPEED, 1) * MAX_STRETCH;
    // 没鼓起来时和格子一样大，鼓满是透镜的大小；横向拉长时纵向收一点，体积大致不变
    const scaleX = lerp(anchor.width / lensWidth, 1, presence) * stretch;
    const scaleY = lerp(anchor.height / lensHeight, 1, presence) / Math.sqrt(stretch);
    const visibleWidth = lensWidth * scaleX;
    const visibleHeight = lensHeight * scaleY;
    const left = motion.x - visibleWidth / 2;
    const top = anchor.centerY - visibleHeight / 2;
    const opacity = Math.min(1, lift * 1.6);

    const element = lens.value;
    element.style.translate = `${motion.x - lensWidth / 2}px ${anchor.centerY - lensHeight / 2}px`;
    element.style.scale = `${scaleX} ${scaleY}`;
    element.style.opacity = opacity;
    // 弯折和透镜一起长出来：没鼓起来时底下的东西原大原位，和水滴交接那一下才对得上
    const strength = Math.round(lift * 100) / 100;
    if (strength !== motion.strength) {
      motion.strength = strength;
      const surface = element.liquidGlassSurface;
      if (surface) setLiquidGlassStrength(surface.id, strength);
    }

    if (tint?.value) {
      tint.value.style.clipPath = `inset(${top}px ${bounds.width - left - visibleWidth}px `
        + `${bounds.height - top - visibleHeight}px ${left}px round ${visibleHeight / 2}px)`;
      tint.value.style.opacity = opacity;
    }
    // 水滴比透镜先一步藏好：鼓到八成就完全看不见，切换时它直接跳到新位置，不会闪一下
    const hidden = Math.min(1, lift * 1.25);
    container.value.style.setProperty("--droplet-opacity", String(1 - hidden));
  }

  function finish() {
    frame = 0;
    lensing.value = false;
    motion.strength = -1;
    container.value?.style.removeProperty("--droplet-opacity");
  }

  onBeforeUnmount(() => cancelAnimationFrame(frame));

  return {
    lensing,
    press,
    move,
    release,
  };
}
