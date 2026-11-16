// The particle canvas: 100 drifting dots that connect when close and push
// away from the cursor. Runs at 30fps, pauses when off-screen or hidden,
// and draws a single static frame under reduced motion.

import { useEffect, useRef } from "react";
import {
  makeParticles,
  step,
  repel,
  connections,
  type Particle,
} from "../utils/particles";

const COUNT = 100;
const CONNECT_DISTANCE = 100;
const REPEL_RADIUS = 150;
const FRAME_MS = 1000 / 30;

export function useParticleCanvas() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");

    let width = 0;
    let height = 0;
    let particles: Particle[] = [];
    let frame = 0;
    let last = 0;
    let running = false;
    let visible = true;
    const mouse = { x: -9999, y: -9999 };

    // The canvas rect is cached rather than measured on every mousemove:
    // getBoundingClientRect() forces a synchronous layout, and doing that on
    // every pointer event is a classic source of jank. It is refreshed on
    // resize and on scroll (the hero is at the top, so scrolling moves it).
    let rect = { left: 0, top: 0 };
    const measure = () => {
      const r = canvas.getBoundingClientRect();
      rect = { left: r.left, top: r.top };
    };

    const resize = () => {
      const box = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      width = Math.max(1, box.width);
      height = Math.max(1, box.height);
      canvas.width = Math.floor(width * dpr);
      canvas.height = Math.floor(height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      particles = makeParticles(COUNT, width, height);
      measure();
      draw();
    };

    const draw = () => {
      ctx.clearRect(0, 0, width, height);

      const pairs = connections(particles, CONNECT_DISTANCE);
      ctx.strokeStyle = "rgba(196, 185, 168, 0.15)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (const [i, j] of pairs) {
        ctx.moveTo(particles[i].x, particles[i].y);
        ctx.lineTo(particles[j].x, particles[j].y);
      }
      ctx.stroke();

      ctx.fillStyle = "rgba(196, 185, 168, 0.9)";
      for (const p of particles) {
        ctx.globalAlpha = p.opacity;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
    };

    const tick = (now: number) => {
      if (!running) return;
      frame = requestAnimationFrame(tick);

      const elapsed = now - last;
      if (elapsed < FRAME_MS) return;
      last = now;

      for (const p of particles) {
        step(p, width, height, elapsed);
        repel(p, mouse.x, mouse.y, REPEL_RADIUS);
      }
      draw();
    };

    const start = () => {
      if (running || reduced.matches) return;
      running = true;
      last = performance.now();
      frame = requestAnimationFrame(tick);
    };

    const stop = () => {
      running = false;
      cancelAnimationFrame(frame);
    };

    const onMouseMove = (event: MouseEvent) => {
      mouse.x = event.clientX - rect.left;
      mouse.y = event.clientY - rect.top;
    };

    const onMouseLeave = () => {
      mouse.x = -9999;
      mouse.y = -9999;
    };

    const onVisibility = () => {
      if (document.visibilityState === "hidden") stop();
      else if (visible) start();
    };

    // Re-init on resize, debounced: a drag-resize would otherwise rebuild
    // the particle field on every frame.
    let resizeTimer: number | undefined;
    const onResize = () => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(resize, 150);
    };

    const observer = new IntersectionObserver(
      ([entry]) => {
        visible = entry.isIntersecting;
        if (visible && document.visibilityState !== "hidden") start();
        else stop();
      },
      { threshold: 0 },
    );

    resize();
    measure();
    observer.observe(canvas);
    window.addEventListener("resize", onResize);
    window.addEventListener("scroll", measure, { passive: true });
    window.addEventListener("mousemove", onMouseMove, { passive: true });
    window.addEventListener("mouseleave", onMouseLeave);
    document.addEventListener("visibilitychange", onVisibility);

    // Reduced motion: one static frame, no loop.
    if (!reduced.matches) start();

    const onReducedChange = () => {
      if (reduced.matches) {
        stop();
        draw();
      } else if (visible) {
        start();
      }
    };
    reduced.addEventListener("change", onReducedChange);

    return () => {
      stop();
      observer.disconnect();
      window.clearTimeout(resizeTimer);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("scroll", measure);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseleave", onMouseLeave);
      document.removeEventListener("visibilitychange", onVisibility);
      reduced.removeEventListener("change", onReducedChange);
    };
  }, []);

  return canvasRef;
}
