// ── Catalog document model ────────────────────────────────────────────────
// A catalog is a free-form, multi-page design document (Canva-style).  Every
// element carries concrete content (real image paths, typed text) — there are
// no templates or data bindings.  Stored as JSON in the catalog store.

export type Align = 'left' | 'center' | 'right';
export type ElementType = 'text' | 'image' | 'box' | 'line';

/** One element on a page.  x/y/w/h are PERCENT of the page. */
export interface DocElement {
  id:   string;
  type: ElementType;
  x: number; y: number; w: number; h: number;

  // text
  content?: string;
  font?:    string;
  size?:    number;   // pt
  bold?:    boolean;
  italic?:  boolean;
  align?:   Align;
  color?:   string;

  // image
  src?:    string;            // absolute file path
  fit?:    'cover' | 'contain';
  radius?: number;           // px

  // box
  fill?:        string;
  borderColor?: string;
  borderWidth?: number;

  // line
  thickness?: number;
  lineColor?: string;
}

export interface PageDoc {
  id:       string;
  bg:       string;          // background colour
  elements: DocElement[];
}

export interface CatalogDoc {
  id:    string;
  name:  string;
  pageW: number;             // mm
  pageH: number;             // mm
  pages: PageDoc[];
}

export interface CatalogSummary { id: string; name: string; updated_at: number; }

// ── Fonts (mapped to jsPDF core families for export) ──────────────────────
export interface FontDef { key: string; label: string; css: string; pdf: 'helvetica' | 'times' | 'courier'; }
export const FONTS: FontDef[] = [
  { key: 'grotesk', label: 'Space Grotesk', css: "'Space Grotesk', sans-serif", pdf: 'helvetica' },
  { key: 'sans',    label: 'Sans',          css: "Arial, Helvetica, sans-serif", pdf: 'helvetica' },
  { key: 'serif',   label: 'Serif',         css: "Georgia, 'Times New Roman', serif", pdf: 'times' },
  { key: 'mono',    label: 'Mono',          css: "'Courier New', monospace", pdf: 'courier' },
];
export const fontCss = (key?: string) => FONTS.find(f => f.key === key)?.css ?? FONTS[0].css;
export const fontPdf = (key?: string) => FONTS.find(f => f.key === key)?.pdf ?? 'helvetica';

// ── Page presets ──────────────────────────────────────────────────────────
export interface PagePreset { key: string; label: string; w: number; h: number; }
export const PAGE_PRESETS: PagePreset[] = [
  { key: 'a4',     label: 'A4 Portrait',  w: 210,   h: 297 },
  { key: 'a4l',    label: 'A4 Landscape', w: 297,   h: 210 },
  { key: 'letter', label: 'Letter',       w: 215.9, h: 279.4 },
  { key: 'square', label: 'Square',       w: 210,   h: 210 },
];

// ── Factories ─────────────────────────────────────────────────────────────
let _seq = 0;
export const uid = () => `${Date.now().toString(36)}${(_seq++).toString(36)}`;

export function blankPage(): PageDoc {
  return { id: uid(), bg: '#ffffff', elements: [] };
}

export function defaultCatalog(name = 'Untitled catalog'): CatalogDoc {
  return { id: uid(), name, pageW: 210, pageH: 297, pages: [blankPage()] };
}

/** Deep-clone a page (new ids) for duplication. */
export function clonePage(p: PageDoc): PageDoc {
  return {
    id: uid(),
    bg: p.bg,
    elements: p.elements.map(e => ({ ...e, id: uid() })),
  };
}
