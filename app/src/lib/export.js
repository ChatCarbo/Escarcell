// Exports : CSV (lisible par Excel en français) et images PNG des graphiques.
import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

const aujourdhui = () => new Date().toISOString().slice(0, 10);

async function enregistrer(nomParDefaut, extension, octets) {
  const chemin = await save({
    defaultPath: `${nomParDefaut}-${aujourdhui()}.${extension}`,
    filters: [{ name: extension === 'csv' ? 'Tableau CSV' : 'Image PNG', extensions: [extension] }],
  });
  if (!chemin) return null;
  await invoke('enregistrer_export', { chemin, octets: Array.from(octets) });
  return chemin;
}

/** Nombre au format français, sans séparateur de milliers (pour Excel). */
export const nombreCsv = (v, decimales = 2) => (v === null || v === undefined ? '' : Number(v).toFixed(decimales).replace('.', ','));
/** Centimes → euros pour le CSV. */
export const eurosCsv = (cents) => nombreCsv(cents === null || cents === undefined ? null : cents / 100);
/** Millionièmes → nombre pour le CSV. */
export const microCsv = (micro, decimales = 4) => nombreCsv(micro === null || micro === undefined ? null : micro / 1e6, decimales);

function cellule(v) {
  const s = v === null || v === undefined ? '' : String(v);
  return /[;"\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/**
 * Exporte un tableau en CSV : séparateur « ; », virgule décimale, UTF-8 avec BOM,
 * pour une ouverture directe dans Excel ou LibreOffice en français.
 */
export async function exporterCsv(nom, entetes, lignes) {
  const texte = [entetes, ...lignes].map((l) => l.map(cellule).join(';')).join('\r\n') + '\r\n';
  const octets = new TextEncoder().encode('﻿' + texte);
  return enregistrer(nom, 'csv', octets);
}

// Propriétés de style recopiées sur chaque élément, pour que l'image ressemble à l'écran.
const STYLES = ['text-anchor', 'dominant-baseline', 'fill', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-dasharray', 'stroke-opacity', 'opacity', 'font-size', 'font-weight', 'font-family', 'paint-order', 'stroke-linejoin', 'visibility'];

function svgAutonome(svg) {
  const copie = svg.cloneNode(true);
  const source = [svg, ...svg.querySelectorAll('*')];
  const cible = [copie, ...copie.querySelectorAll('*')];
  source.forEach((el, i) => {
    const cs = getComputedStyle(el);
    cible[i].setAttribute('style', STYLES.map((p) => `${p}:${cs.getPropertyValue(p)}`).join(';'));
  });
  const vb = svg.viewBox.baseVal;
  copie.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  copie.setAttribute('width', vb.width);
  copie.setAttribute('height', vb.height);
  return { texte: new XMLSerializer().serializeToString(copie), largeur: vb.width, hauteur: vb.height };
}

function charger(texte) {
  return new Promise((ok, ko) => {
    const img = new Image();
    img.onload = () => ok(img);
    img.onerror = () => ko(new Error("Impossible de dessiner le graphique."));
    img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(texte);
  });
}

/**
 * Exporte un ou plusieurs SVG, empilés, en une image PNG (résolution doublée),
 * avec un titre et une ligne de bas de page.
 */
export async function exporterImage(nom, titre, svgs, piedDePage = '') {
  const parties = svgs.filter(Boolean).map(svgAutonome);
  if (!parties.length) return null;
  const largeur = Math.max(...parties.map((p) => p.largeur));
  const marge = 16, hautTitre = titre ? 36 : 0, basPied = piedDePage ? 24 : 0, espace = 12;
  const hauteur = hautTitre + parties.reduce((s, p) => s + (p.hauteur * largeur) / p.largeur + espace, 0) + basPied;
  const echelle = 2;
  const canvas = document.createElement('canvas');
  canvas.width = (largeur + 2 * marge) * echelle;
  canvas.height = (hauteur + 2 * marge) * echelle;
  const ctx = canvas.getContext('2d');
  ctx.scale(echelle, echelle);
  const racine = getComputedStyle(document.documentElement);
  ctx.fillStyle = racine.getPropertyValue('--surface').trim() || '#ffffff';
  ctx.fillRect(0, 0, largeur + 2 * marge, hauteur + 2 * marge);
  ctx.fillStyle = racine.getPropertyValue('--texte').trim() || '#0f172a';
  let y = marge;
  if (titre) {
    ctx.font = '600 18px Inter, "Segoe UI", sans-serif';
    ctx.fillText(titre, marge, y + 20);
    y += hautTitre;
  }
  for (const p of parties) {
    const img = await charger(p.texte);
    const h = (p.hauteur * largeur) / p.largeur;
    ctx.drawImage(img, marge, y, largeur, h);
    y += h + espace;
  }
  if (piedDePage) {
    ctx.font = '11px Inter, "Segoe UI", sans-serif';
    ctx.fillStyle = racine.getPropertyValue('--discret').trim() || '#64748b';
    ctx.fillText(piedDePage, marge, y + 12);
  }
  const blob = await new Promise((ok) => canvas.toBlob(ok, 'image/png'));
  const octets = new Uint8Array(await blob.arrayBuffer());
  return enregistrer(nom, 'png', octets);
}

/** Nom de fichier sans caractères gênants. */
export const nomFichier = (s) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9]+/g, '-').replace(/^-|-$/g, '').toLowerCase();
