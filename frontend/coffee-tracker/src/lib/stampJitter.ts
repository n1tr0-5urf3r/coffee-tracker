export type StampJitter = {
  rotation: number
  offsetX: number
  offsetY: number
  scale: number
  inkOpacity: number
}

/** Small deterministic hash so a stamp keeps its imperfections across renders. */
function hash(seed: number, salt: number) {
  const value = Math.sin(seed * 12.9898 + salt * 78.233) * 43758.5453
  return value - Math.floor(value)
}

/**
 * Every imprint sits slightly differently on the paper: a real rubber stamp is
 * never applied twice at the same angle, pressure or ink coverage.
 */
export function stampJitter(seed: number): StampJitter {
  return {
    rotation: -14 + hash(seed, 1) * 28,
    offsetX: -2 + hash(seed, 2) * 4,
    offsetY: -2 + hash(seed, 3) * 4,
    scale: 0.94 + hash(seed, 4) * 0.1,
    inkOpacity: 0.72 + hash(seed, 5) * 0.24,
  }
}
