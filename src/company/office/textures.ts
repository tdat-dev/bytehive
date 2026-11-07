/* The two pictures in the office that come from files: the honey cell on the roof, and
 * the CLI logo on each badge. They arrive after the first frame, so `loaded` asks for one. */
import { CanvasTexture, SRGBColorSpace, TextureLoader, type Texture } from "three";
import { cliLogoUrl } from "../../clibrand";

const LOGO_SIZE = 512;
const LETTER_SIZE = 64;
const INK = "#14161c";
/** Logos drawn in white, for a dark tile: on the white card they take the ink instead. */
const WHITE_LOGOS = new Set(["amp", "cline", "copilot", "cursor", "goose", "grok", "hermes", "kilo", "kimi", "ollama", "opencode", "roo"]);

/** design/logo.svg, byte for byte: a test holds it to the file. A template literal reads
 *  its line ends as LF whatever is in this file, and the logo's are CRLF. */
export const LOGO_SVG = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="512" height="512">
  <!-- ByteHive: a honey cell with a faint comb, a terminal prompt cut out of it. -->
  <defs>
    <linearGradient id="honey" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffd85c"/>
      <stop offset=".5" stop-color="#f5a524"/>
      <stop offset="1" stop-color="#c86a12"/>
    </linearGradient>
    <pattern id="comb" width="4.5033" height="7.8" patternUnits="userSpaceOnUse" x="9.748" y="9.4">
      <path d="M2.2517 0 4.5033 1.3V3.9L2.2517 5.2 0 3.9V1.3ZM2.2517 5.2V7.8" fill="none" stroke="#0e0f12" stroke-opacity=".22" stroke-width=".32"/>
    </pattern>
    <mask id="prompt">
      <rect width="24" height="24" fill="#fff"/>
      <g fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round">
        <path d="M8 8.8 11.39 12.19 8 15.58"/>
        <path d="M13.65 15.58H17.49"/>
      </g>
    </mask>
  </defs>
  <g mask="url(#prompt)">
    <path d="M12 .8 21.699 6.4V17.6L12 23.2 2.301 17.6V6.4Z" fill="url(#honey)"/>
    <path d="M12 .8 21.699 6.4V17.6L12 23.2 2.301 17.6V6.4Z" fill="url(#comb)"/>
  </g>
</svg>
`.replace(/\n/g, "\r\n");

/** What goes on a badge for a CLI with no logo. */
export function badgeLetter(cli: string): string {
  return /[\p{L}\p{N}]/u.exec(cli)?.[0].toUpperCase() ?? "?";
}

/** The colour a logo is multiplied by on the white card. */
export const badgeInk = (cli: string): string => (WHITE_LOGOS.has(cli) ? INK : "#ffffff");

function letterTexture(cli: string): Texture {
  const canvas = new OffscreenCanvas(LETTER_SIZE, LETTER_SIZE);
  const c = canvas.getContext("2d")!;
  c.fillStyle = INK;
  c.font = "700 46px sans-serif";
  c.textAlign = "center";
  c.textBaseline = "middle";
  c.fillText(badgeLetter(cli), LETTER_SIZE / 2, LETTER_SIZE / 2 + 2);
  const t = new CanvasTexture(canvas);
  t.colorSpace = SRGBColorSpace;
  return t;
}

export interface OfficeTextures {
  logoTexture(): Texture;
  badgeTexture(cli: string): Texture;
  dispose(): void;
}

/** One scene's textures, so a logo that loads late wakes the scene that asked for it. */
export function createTextures(loaded: () => void): OfficeTextures {
  const badges = new Map<string, Texture>();
  const loader = new TextureLoader();
  let logo: CanvasTexture | null = null;

  function logoTexture(): Texture {
    if (logo) return logo;
    const canvas = new OffscreenCanvas(LOGO_SIZE, LOGO_SIZE);
    const t = new CanvasTexture(canvas);
    t.colorSpace = SRGBColorSpace;
    t.anisotropy = 4;
    logo = t;
    const image = new Image();
    image.onload = () => {
      canvas.getContext("2d")!.drawImage(image, 0, 0, LOGO_SIZE, LOGO_SIZE);
      t.needsUpdate = true;
      loaded();
    };
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(LOGO_SVG)}`;
    return t;
  }

  function badgeTexture(cli: string): Texture {
    const known = badges.get(cli);
    if (known) return known;
    const url = cliLogoUrl({ badge: cli });
    const t = url ? loader.load(url, loaded) : letterTexture(cli);
    t.colorSpace = SRGBColorSpace;
    badges.set(cli, t);
    return t;
  }

  return {
    logoTexture,
    badgeTexture,
    dispose() {
      logo?.dispose();
      for (const t of badges.values()) t.dispose();
      badges.clear();
    },
  };
}
