// Saving results: files are built in the browser and handed to the browser's download, nothing is uploaded.

export function download(name: string, data: Blob | string, type = 'text/plain') {
  const blob = typeof data === 'string' ? new Blob([data], { type }) : data;
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

const cell = (v: string | number) => {
  const s = typeof v === 'number' ? (Number.isFinite(v) ? String(v) : '') : v;
  return /[",\n]/.test(s) ? `"${s.replaceAll('"', '""')}"` : s;
};

export const csv = (rows: (string | number)[][]) => rows.map((r) => r.map(cell).join(',')).join('\n') + '\n';

export const canvasBlob = (c: HTMLCanvasElement) =>
  new Promise<Blob>((resolve, reject) => c.toBlob((b) => (b ? resolve(b) : reject(new Error('PNG export failed'))), 'image/png'));

/** Rasterise a standalone SVG document (with width and height attributes) at `scale` times its size. */
export async function svgToPng(svg: string, width: number, height: number, scale = 2): Promise<Blob> {
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = width * scale;
    c.height = height * scale;
    const ctx = c.getContext('2d')!;
    ctx.fillStyle = '#fff';
    ctx.fillRect(0, 0, c.width, c.height);
    ctx.drawImage(img, 0, 0, c.width, c.height);
    return await canvasBlob(c);
  } finally {
    URL.revokeObjectURL(url);
  }
}

export const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'graph';
