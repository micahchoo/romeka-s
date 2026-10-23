/**
 * The Spanish words of the Structure screen's corrections, written by hand.
 *
 * It is typed `ImportFixesMessages`, so a key added to English and not here is
 * a `tsc` error.
 */
import type { ImportFixesMessages } from "../index";

export const es: ImportFixesMessages = {
  "fix.q.items": "Oferta: elementos",
  "fix.q.rows": "Oferta: filas",
  "fix.oneItem": "{count} elementos podrían ir dentro de otro elemento.",
  "fix.oneItemOne": "1 elemento podría ir dentro de otro elemento.",
  "fix.rowIs": "{count} filas no nombran ningún archivo.",
  "fix.rowIsOne": "1 fila no nombra ningún archivo.",
  "fix.rowIs.note": "Cada una apunta a un archivo que no tiene fila.",
  "fix.rowIs.noteOne": "Apunta a un archivo que no tiene fila.",
  "fix.describesNothing":
    "{count} filas no nombran ningún archivo, y ninguno se parece.",
  "fix.describesNothingOne":
    "1 fila no nombra ningún archivo, y ninguno se parece.",
  "fix.describesNothing.note":
    "Cada una crea un elemento sin archivo.",
  "fix.describesNothing.noteOne": "Crea un elemento sin archivo.",
  "fix.items": "Elementos: {before} → {after}",
  "fix.rows":
    "Filas que encuentran un archivo: {before} → {after} de {of}",
  "fix.show": "Ver las {count}",
  "fix.showOne": "Verla",
  "fix.oneItem.take": "Poner estos {count} en su elemento",
  "fix.oneItem.takeOne": "Ponerlo en su elemento",
  "fix.rowIs.take": "Emparejar estas {count}",
  "fix.rowIs.takeOne": "Emparejar esta fila",
  "fix.describesNothing.take": "Dejar fuera estas {count}",
  "fix.describesNothing.takeOne": "Dejar fuera esta fila",
  "fix.join": "{with} va dentro del elemento de {item}",
  "fix.pair": "Fila {line}, {cell}: archivo {file}",
  "fix.leave": "Fila {line}, {cell}",
  "fix.because.keyExtends":
    "Su nombre empieza por el nombre del otro elemento.",
  "fix.because.rowNamesBoth": "Una fila nombra a los dos.",
  "fix.because.near": "Los nombres difieren en un carácter.",
  "fix.because.namesItem":
    "La fila nombra el elemento de este archivo.",
  "fix.because.namesPart":
    "La fila nombra la parte de su elemento que es este archivo.",
  "fix.joined": "Dentro de otro elemento: {count}",
  "fix.paired": "Emparejadas: {count}",
  "fix.left": "Dejadas fuera: {count}",
  "fix.stale": "{count} ya no se aplican.",
  "fix.staleOne": "1 ya no se aplica.",
  "fix.rowChanged": "Esta fila cambió.",
  "fix.fileGone": "Este archivo ya no está.",
};
