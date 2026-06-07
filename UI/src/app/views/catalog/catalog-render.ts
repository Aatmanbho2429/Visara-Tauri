// ── Catalog export engine ─────────────────────────────────────────────────
// Resolves a CatalogDoc's image elements to data URLs and renders the document
// to a PDF via jsPDF (vector text + embedded images).

import { jsPDF } from 'jspdf';
import { CatalogDoc, DocElement, fontPdf } from '../../models/catalog.model';

export interface ResolvedImage { dataUrl: string; w: number; h: number; }

export interface RElement {
  type: 'text' | 'image' | 'box' | 'line';
  x: number; y: number; w: number; h: number;
  text?: string; font?: string; size?: number; bold?: boolean; align?: string; color?: string;
  img?: ResolvedImage | null; fit?: string; radius?: number;
  fill?: string; borderColor?: string; borderWidth?: number;
  thickness?: number; lineColor?: string;
}
export interface RPage { bg: string; elements: RElement[]; }

type ImageResolver = (path: string) => Promise<ResolvedImage | null>;

/** Resolve a document's images and flatten it into renderable pages. */
export async function resolveDoc(doc: CatalogDoc, resolveImg: ImageResolver): Promise<RPage[]> {
  const cache = new Map<string, ResolvedImage | null>();
  const out: RPage[] = [];

  for (const page of doc.pages) {
    const els: RElement[] = [];
    for (const el of page.elements) {
      els.push(await toR(el, cache, resolveImg));
    }
    out.push({ bg: page.bg || '#ffffff', elements: els });
  }
  return out;
}

async function toR(el: DocElement, cache: Map<string, ResolvedImage | null>, resolveImg: ImageResolver): Promise<RElement> {
  const r: RElement = { type: el.type, x: el.x, y: el.y, w: el.w, h: el.h };
  if (el.type === 'image') {
    r.fit = el.fit ?? 'cover';
    r.radius = el.radius ?? 0;
    if (el.src) {
      if (!cache.has(el.src)) {
        try { cache.set(el.src, await resolveImg(el.src)); } catch { cache.set(el.src, null); }
      }
      r.img = cache.get(el.src) ?? null;
    }
  } else if (el.type === 'box') {
    r.fill = el.fill ?? '#eeeeee';
    r.borderColor = el.borderColor;
    r.borderWidth = el.borderWidth ?? 0;
    r.radius = el.radius ?? 0;
  } else if (el.type === 'line') {
    r.thickness = el.thickness ?? 2;
    r.lineColor = el.lineColor ?? '#333333';
  } else {
    r.text = el.content ?? '';
    r.font = el.font; r.size = el.size ?? 12; r.bold = !!el.bold;
    r.align = el.align ?? 'left'; r.color = el.color ?? '#333333';
  }
  return r;
}

// ── PDF ─────────────────────────────────────────────────────────────────────
/** Build the PDF and return base64 (no prefix) for the backend to write. */
export function pdfBase64(pages: RPage[], wMm: number, hMm: number): string {
  const orientation = wMm > hMm ? 'landscape' : 'portrait';
  const doc = new jsPDF({ unit: 'mm', format: [wMm, hMm], orientation });

  pages.forEach((page, i) => {
    if (i > 0) doc.addPage([wMm, hMm], orientation);
    doc.setFillColor(page.bg || '#ffffff');
    doc.rect(0, 0, wMm, hMm, 'F');

    for (const el of page.elements) {
      const x = (el.x / 100) * wMm, y = (el.y / 100) * hMm;
      const w = (el.w / 100) * wMm, h = (el.h / 100) * hMm;

      if (el.type === 'image' && el.img) {
        const ar = el.img.w / el.img.h;
        let dw = w, dh = w / ar;
        if (el.fit === 'cover') {
          // fill the slot (may crop vertically/horizontally) — jsPDF can't clip,
          // so approximate by filling the box; minor for square-ish tiles.
          dw = w; dh = h;
        } else {
          if (dh > h) { dh = h; dw = h * ar; }
        }
        const dx = el.fit === 'cover' ? x : x + (w - dw) / 2;
        const dy = el.fit === 'cover' ? y : y + (h - dh) / 2;
        try { doc.addImage(el.img.dataUrl, 'JPEG', dx, dy, dw, dh); } catch { /* skip */ }
      } else if (el.type === 'box') {
        doc.setFillColor(el.fill || '#eeeeee');
        if (el.borderWidth && el.borderColor) {
          doc.setDrawColor(el.borderColor); doc.setLineWidth(el.borderWidth * 0.3528);
          doc.rect(x, y, w, h, 'FD');
        } else {
          doc.rect(x, y, w, h, 'F');
        }
      } else if (el.type === 'line') {
        doc.setDrawColor(el.lineColor || '#333333');
        doc.setLineWidth((el.thickness ?? 2) * 0.3528);
        doc.line(x, y + h / 2, x + w, y + h / 2);
      } else if (el.type === 'text' && el.text) {
        doc.setFont(fontPdf(el.font), el.bold ? 'bold' : 'normal');
        doc.setFontSize(el.size ?? 12);
        doc.setTextColor(el.color || '#333333');
        const tx = el.align === 'center' ? x + w / 2 : el.align === 'right' ? x + w : x;
        doc.text(el.text, tx, y, { align: (el.align as any) ?? 'left', maxWidth: w, baseline: 'top' });
      }
    }
  });

  const datauri = doc.output('datauristring') as string;
  return datauri.split('base64,')[1] ?? '';
}

// ── Async image helpers ────────────────────────────────────────────────────
export function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result as string);
    r.onerror = reject;
    r.readAsDataURL(blob);
  });
}

export function imageDims(dataUrl: string): Promise<{ w: number; h: number }> {
  return new Promise(resolve => {
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth || 1, h: img.naturalHeight || 1 });
    img.onerror = () => resolve({ w: 1, h: 1 });
    img.src = dataUrl;
  });
}
