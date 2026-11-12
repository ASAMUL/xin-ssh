// 在一条 Lottie 时间线上按 marker 切状态（打盹的猫：nap ⇄ awake）。
// 每个状态是一段循环；两个状态之间可以插一段只播一次的过渡（wake / sleep）。
//
// transitions: [{ from, to, via, finishLoop }]
//   via         过渡段的 marker
//   finishLoop  先把当前这一轮循环播完再走：醒着的猫甩尾巴甩到一半直接跳去睡，尾巴会闪一下；
//               反过来惊醒就该立刻走，那一下跳动正好读成「被吓了一跳」
//
// 过渡途中目标又变了（CPU 在 80% 上下来回跳）不打断，落地以后再按新目标走，猫不会抽搐。
export function createMarkerSequencer(player, { initial, transitions = [], animate = true }) {
  const starts = new Map(player.markers().map(({ name, start }) => [name, start]));
  // looping：在 marker 上循环；leaving：等这一轮播完再出发；passing：正在播过渡，落地后到 to
  let state = { phase: "looping", marker: initial };
  let target = initial;
  let motion = animate;

  function route(from, to) {
    return transitions.find((transition) => transition.from === from && transition.to === to) ?? null;
  }

  function playSegment(marker, loop) {
    player.setMarker(marker);
    player.setLoop(loop);
    player.setFrame(starts.get(marker) ?? 0);
    player.play();
  }

  // 减少动态效果：不播过渡，直接停在目标状态的第一帧
  function rest(marker) {
    state = { phase: "looping", marker };
    player.setMarker(marker);
    player.setFrame(starts.get(marker) ?? 0);
    player.pause();
  }

  function loop(marker) {
    state = { phase: "looping", marker };
    playSegment(marker, true);
  }

  function depart(from) {
    const transition = route(from, target);
    if (!transition) {
      loop(target);
      return;
    }
    state = { phase: "passing", marker: transition.via, to: target };
    playSegment(transition.via, false);
  }

  // 循环每转完一圈触发：等着离开的状态在这里出发
  function onLoop() {
    if (state.phase !== "leaving") return;
    if (target === state.marker) {
      state = { phase: "looping", marker: target };
      return;
    }
    depart(state.marker);
  }

  // 过渡段（不循环）播完触发
  function onComplete() {
    if (state.phase !== "passing") return;
    const arrived = state.to;
    if (target === arrived) loop(arrived);
    else depart(arrived);
  }

  player.addEventListener("loop", onLoop);
  player.addEventListener("complete", onComplete);

  function goTo(next) {
    target = next;
    if (!motion) {
      rest(next);
      return;
    }
    if (state.phase !== "looping") {
      // 正等着离开时目标又回来了：接着循环就行
      if (state.phase === "leaving" && state.marker === next) state = { phase: "looping", marker: next };
      return;
    }
    if (state.marker === next) return;

    if (route(state.marker, next)?.finishLoop) {
      state = { phase: "leaving", marker: state.marker };
      return;
    }
    depart(state.marker);
  }

  function setMotion(enabled) {
    motion = enabled;
    if (enabled) loop(target);
    else rest(target);
  }

  function dispose() {
    player.removeEventListener("loop", onLoop);
    player.removeEventListener("complete", onComplete);
  }

  if (!motion) rest(initial);

  return { goTo, setMotion, dispose };
}
