// The drawing tool's vector scene (DrawingCanvas.vue): what actually gets
// saved as the `<uuid>.drawing.json` sidecar next to a drawing's rendered
// `<uuid>.drawing.png` (see FileAttachmentView.vue) - kept re-editable
// without needing to decode the raster image back into shapes.

export type Tool = "pen" | "rect" | "ellipse" | "line" | "text" | "eraser";

export type Shape =
  | { type: "path"; points: [number, number][]; color: string; width: number }
  | { type: "rect" | "ellipse" | "line"; x1: number; y1: number; x2: number; y2: number; color: string; width: number }
  | { type: "text"; x: number; y: number; text: string; color: string; fontSize: number };

export function drawShape(ctx: CanvasRenderingContext2D, shape: Shape) {
  ctx.strokeStyle = shape.type === "text" ? "transparent" : shape.color;
  ctx.fillStyle = shape.color;

  switch (shape.type) {
    case "path": {
      if (shape.points.length === 0) return;
      ctx.lineWidth = shape.width;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(...shape.points[0]);
      for (const [x, y] of shape.points.slice(1)) ctx.lineTo(x, y);
      ctx.stroke();
      break;
    }
    case "line": {
      ctx.lineWidth = shape.width;
      ctx.lineCap = "round";
      ctx.beginPath();
      ctx.moveTo(shape.x1, shape.y1);
      ctx.lineTo(shape.x2, shape.y2);
      ctx.stroke();
      break;
    }
    case "rect": {
      ctx.lineWidth = shape.width;
      ctx.strokeRect(
        Math.min(shape.x1, shape.x2),
        Math.min(shape.y1, shape.y2),
        Math.abs(shape.x2 - shape.x1),
        Math.abs(shape.y2 - shape.y1),
      );
      break;
    }
    case "ellipse": {
      ctx.lineWidth = shape.width;
      const cx = (shape.x1 + shape.x2) / 2;
      const cy = (shape.y1 + shape.y2) / 2;
      const rx = Math.abs(shape.x2 - shape.x1) / 2;
      const ry = Math.abs(shape.y2 - shape.y1) / 2;
      ctx.beginPath();
      ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
      ctx.stroke();
      break;
    }
    case "text": {
      ctx.font = `${shape.fontSize}px sans-serif`;
      ctx.textBaseline = "alphabetic";
      ctx.fillText(shape.text, shape.x, shape.y);
      break;
    }
  }
}
