// The ByteHive logo (design/logo.svg) with its comb cells ready to light up.
// The cells stay dark until CSS animates `.hc`; `--n` is each one's turn.

const HEX = "M12 .8 21.699 6.4V17.6L12 23.2 2.301 17.6V6.4Z";

/** Comb cells in the order they light up, as [column, row]: the middle one,
 *  the ring around it, then the outer cells. */
export const CELLS: [number, number][] = [
  [0, 0], [1, 0], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1],
  [1, -1], [1, 1], [0, 2], [-2, 0], [0, -2], [2, 0],
];

/** A comb cell's outline. Odd rows sit half a cell to the right. */
export function cellPath(col: number, row: number): string {
  const x = +(12 + 4.5033 * (col + 0.5 * Math.abs(row % 2))).toFixed(4);
  const y = +(12 + 3.9 * row).toFixed(4);
  const [l, r] = [+(x - 2.2517).toFixed(4), +(x + 2.2517).toFixed(4)];
  return `M${x} ${+(y - 2.6).toFixed(4)} ${r} ${+(y - 1.3).toFixed(4)}V${+(y + 1.3).toFixed(4)}L${x} ${+(y + 2.6).toFixed(4)} ${l} ${+(y + 1.3).toFixed(4)}V${+(y - 1.3).toFixed(4)}Z`;
}

/** The inside of the logo's <svg>; `id` keeps its defs apart from another's. */
export function hiveLogo(id: string): string {
  const cells = CELLS.map(([c, r], n) => `<path class="hc" style="--n:${n}" d="${cellPath(c, r)}"/>`).join("");
  return `<defs><linearGradient id="${id}Honey" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffd85c"/><stop offset=".5" stop-color="#f5a524"/><stop offset="1" stop-color="#c86a12"/></linearGradient>`
    + `<pattern id="${id}Comb" width="4.5033" height="7.8" patternUnits="userSpaceOnUse" x="9.748" y="9.4"><path d="M2.2517 0 4.5033 1.3V3.9L2.2517 5.2 0 3.9V1.3ZM2.2517 5.2V7.8" fill="none" stroke="#0e0f12" stroke-opacity=".22" stroke-width=".32"/></pattern>`
    + `<clipPath id="${id}Clip"><path d="${HEX}"/></clipPath>`
    + `<mask id="${id}Prompt"><rect width="24" height="24" fill="#fff"/><g fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><path d="M8 8.8 11.39 12.19 8 15.58"/><path d="M13.65 15.58H17.49"/></g></mask></defs>`
    + `<g mask="url(#${id}Prompt)"><path d="${HEX}" fill="url(#${id}Honey)"/><path d="${HEX}" fill="url(#${id}Comb)"/><g clip-path="url(#${id}Clip)">${cells}</g></g>`;
}
