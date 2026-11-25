// Conversions entre l'affichage (français) et les unités entières du cœur Rust :
// centimes pour les montants, millionièmes pour les taux, quantités et cours.

const fmtEuros = new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' });

export const euros = (cents) => fmtEuros.format((cents ?? 0) / 100);

export const pct = (micro, decimales = 2) =>
  new Intl.NumberFormat('fr-FR', {
    style: 'percent',
    minimumFractionDigits: decimales,
    maximumFractionDigits: decimales,
  }).format((micro ?? 0) / 1e6);

const nombre = (s) => {
  const n = Number(String(s ?? '').replace(/[\s  €%]/g, '').replace(',', '.'));
  return String(s ?? '').trim() !== '' && Number.isFinite(n) ? n : null;
};

/** "1 234,56" → 123456 */
export const versCents = (s) => (nombre(s) === null ? null : Math.round(nombre(s) * 100));
/** "5,0199" → 5019900 (cours ou quantité en millionièmes) */
export const versMicro = (s) => (nombre(s) === null ? null : Math.round(nombre(s) * 1e6));
/** "1,5" (%) → 15000 */
export const pctVersMicro = (s) => (nombre(s) === null ? null : Math.round(nombre(s) * 1e4));

export const microVersTexte = (m) =>
  m === null || m === undefined ? '' : (m / 1e6).toLocaleString('fr-FR', { maximumFractionDigits: 6 });

/** Date du jour au format AAAA-MM-JJ, à l'heure locale (toISOString donnerait la date UTC : la veille entre minuit et 1 h ou 2 h en France). */
export const aujourdHui = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
};
