/**
 * The Spanish fixity check messages, written by hand.
 *
 * It is typed `FixityMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { FixityMessages } from '../index';

export const es: FixityMessages = {
  'fixity.class': 'Comprobar los archivos guardados',
  'fixity.start': 'Comprobar los archivos guardados',
  'fixity.start.note':
    'Lee cada original guardado y lo compara con el SHA-256 registrado cuando se añadió el archivo.',
  'fixity.start.busy': 'Iniciando la comprobación…',
  'fixity.start.failed': 'La comprobación no se inició: {message}',
  'fixity.result.title': 'Resultado',
  'fixity.result.loading': 'Cargando el resultado…',
  'fixity.result.failed': 'No se pudo leer el resultado: {message}',
  'fixity.result.waiting': 'El resultado aparece aquí cuando termina la comprobación.',
  'fixity.result.none':
    'Esta comprobación terminó antes de escribir un resultado. Su registro dice por qué.',
  'fixity.result.checkedOne': '1 archivo comprobado el {date}.',
  'fixity.result.checkedMany': '{count} archivos comprobados el {date}.',
  'fixity.result.differsOne': '1 es distinto.',
  'fixity.result.differsMany': '{count} son distintos.',
  'fixity.result.missingOne': '1 falta.',
  'fixity.result.missingMany': '{count} faltan.',
  'fixity.result.unreadableOne': '1 no se puede leer.',
  'fixity.result.unreadableMany': '{count} no se pueden leer.',
  'fixity.result.allMatch': 'Todos coinciden.',
  'fixity.result.stopped': 'La comprobación se detuvo antes del último archivo.',
  'fixity.line.problemsOne': 'Archivos comprobados por última vez el {date}: 1 problema',
  'fixity.line.problemsMany': 'Archivos comprobados por última vez el {date}: {count} problemas',
  'fixity.line.noProblems': 'Archivos comprobados por última vez el {date}: ningún problema',
  'fixity.line.never': 'Los archivos guardados nunca se han comprobado.',
  'fixity.line.running': 'Comprobando ahora los archivos guardados.',
  'fixity.filter.checked': 'Archivos leídos por la comprobación n.º {job}.',
  'fixity.filter.ok': 'Archivos que coincidieron en la comprobación n.º {job}.',
  'fixity.filter.differs': 'Archivos distintos en la comprobación n.º {job}.',
  'fixity.filter.missing': 'Archivos que faltan en la comprobación n.º {job}.',
  'fixity.filter.unreadable': 'Archivos que no se pudieron leer en la comprobación n.º {job}.',
  'fixity.filter.back': 'Abrir la comprobación',
  'fixity.filter.clear': 'Mostrar todos los medios',
};
