(() => {
  'use strict';

  const canvas = document.getElementById('theme-texture');
  if (!(canvas instanceof HTMLCanvasElement)) return;
  const context = canvas.getContext('2d');
  if (!context) return;

  const root = document.documentElement;
  const surfaceCanvas = document.createElement('canvas');
  surfaceCanvas.width = 40;
  surfaceCanvas.height = 40;
  const surfaceContext = surfaceCanvas.getContext('2d');
  const surfaceImage = surfaceContext?.createImageData(surfaceCanvas.width, surfaceCanvas.height);
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  const maxSpeed = 100 / 3; // plug1's 100% Motion relative to its 3% default
  const palette = {
    neon: {
      tones: [[4, 15, 16], [6, 25, 26], [8, 38, 37], [13, 54, 47], [24, 72, 51]],
      glint: [178, 248, 87],
    },
    spectral: {
      tones: [[3, 11, 26], [5, 21, 42], [8, 35, 68], [11, 53, 93], [19, 77, 119]],
      glint: [211, 251, 255],
    },
  };

  // A repeatable height map gives the strata and glints stable spatial anchors.
  const lattice = new Float32Array(32 * 32);
  let seed = 7831;
  for (let i = 0; i < lattice.length; i++) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    lattice[i] = seed / 4294967296;
  }

  const mix = (a, b, amount) => a + (b - a) * amount;
  const smooth = (t) => t * t * (3 - 2 * t);
  const sample = (x, y) => lattice[(y & 31) * 32 + (x & 31)];

  function relief(x, y) {
    const ix = Math.floor(x);
    const iy = Math.floor(y);
    const u = smooth(x - ix);
    const v = smooth(y - iy);
    return mix(mix(sample(ix, iy), sample(ix + 1, iy), u),
      mix(sample(ix, iy + 1), sample(ix + 1, iy + 1), u), v);
  }

  let image;
  let seconds = 0;
  let lastTime;
  let lastFrame = -1;
  let lastSurfaceFrame = -1;
  let request = null;
  let dirty = true;
  let surfaceDirty = true;

  function draw() {
    const { clientWidth, clientHeight } = canvas;
    if (!clientWidth || !clientHeight) return;
    const scale = Math.min(1, 80 / Math.max(clientWidth, clientHeight));
    const width = Math.max(1, Math.round(clientWidth * scale));
    const height = Math.max(1, Math.round(clientHeight * scale));
    if (!image || image.width !== width || image.height !== height) {
      canvas.width = width;
      canvas.height = height;
      image = context.createImageData(width, height);
    }

    paint(context, image, seconds, 255);
  }

  function drawSurface() {
    paint(surfaceContext, surfaceImage, seconds * 0.5, 36);
    // The image is not seamless; stretch it over the element rather than tiling it.
    root.style.setProperty('--surface-texture', `url("${surfaceCanvas.toDataURL('image/png')}")`);
  }

  function paint(target, pixels, phase, alpha) {
    const { width, height, data } = pixels;
    const { tones, glint } = root.dataset.theme === 'spectral' ? palette.spectral : palette.neon;
    const drift = Math.floor(phase * maxSpeed * 10) * 0.003;
    for (let y = 0, offset = 0; y < height; y++) {
      const v = y / height;
      for (let x = 0; x < width; x++, offset += 4) {
        const u = x / width;
        const cloud = relief(u * 3.4 + drift * 0.32, v * 3.4 + drift * 0.21);
        const mineral = sample(13 + Math.floor(x / 5), 21 + Math.floor(y / 5));
        const ridge = Math.sin(v * 10 + u * 2.5 + cloud * 4 - drift);
        const level = Math.floor(cloud * 3.6 + mineral * 1.4 + ridge * 0.5);
        const tone = tones[Math.max(0, Math.min(4, level))];

        const fleck = ((Math.imul(x + 1, 0x9e3779b1) ^ Math.imul(y + 1, 0x85ebca77)) >>> 0);
        const gleam = (fleck & 1023) < 2
          ? 0.65 * (0.5 + 0.5 * Math.sin(drift * 4 + (fleck >>> 24) / 40)) ** 2
          : 0;
        for (let channel = 0; channel < 3; channel++) {
          data[offset + channel] = mix(tone[channel], glint[channel], gleam);
        }
        data[offset + 3] = alpha;
      }
    }
    target.putImageData(pixels, 0, 0);
  }

  function schedule() {
    if (request === null && !document.hidden) request = requestAnimationFrame(tick);
  }

  function invalidate() {
    dirty = true;
    schedule();
  }

  function invalidateBoth() {
    surfaceDirty = true;
    invalidate();
  }

  function tick(now) {
    request = null;
    if (document.hidden) return;
    const moving = !reducedMotion.matches;
    if (moving && lastTime !== undefined) seconds += (now - lastTime) / 1000;
    lastTime = moving ? now : undefined;
    // Match the source editor's 30 Hz repaint budget even on fast displays.
    const frame = Math.floor(seconds * 30);
    if (dirty || frame !== lastFrame) {
      draw();
      dirty = false;
      lastFrame = frame;
    }
    const surfaceFrame = Math.floor(seconds * 15);
    if (surfaceImage && (surfaceDirty || surfaceFrame !== lastSurfaceFrame)) {
      drawSurface();
      surfaceDirty = false;
      lastSurfaceFrame = surfaceFrame;
    }
    if (moving) schedule();
  }

  new ResizeObserver(invalidate).observe(canvas);
  new MutationObserver(invalidateBoth).observe(root, { attributes: true, attributeFilter: ['data-theme'] });
  reducedMotion.addEventListener('change', () => {
    lastTime = undefined;
    invalidateBoth();
  });
  document.addEventListener('visibilitychange', () => {
    lastTime = undefined;
    if (document.hidden) {
      if (request !== null) cancelAnimationFrame(request);
      request = null;
    } else {
      invalidateBoth();
    }
  });
  schedule();
})();
