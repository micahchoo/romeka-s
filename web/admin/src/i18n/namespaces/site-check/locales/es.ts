/**
 * The Spanish site check messages, written by hand.
 *
 * It is typed `SiteCheckMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { SiteCheckMessages } from '../index';

export const es: SiteCheckMessages = {
  'siteCheck.title': 'Antes de compartir este sitio',
  'siteCheck.intro': 'Cuenta lo que encuentran los visitantes en las páginas públicas de este sitio.',
  'siteCheck.run': 'Comprobar este sitio',
  'siteCheck.again': 'Comprobar de nuevo',
  'siteCheck.running': 'Comprobando el sitio…',
  'siteCheck.nothing': 'Nada que comprobar.',
  'siteCheck.unwrittenOne': '1 bloque sin escribir, en 1 página pública',
  'siteCheck.unwrittenOnePage': '{count} bloques sin escribir, en 1 página pública',
  'siteCheck.unwritten': '{count} bloques sin escribir, en {pages} páginas públicas',
  'siteCheck.emptyListsOne': '1 lista no muestra nada a los visitantes, en 1 página pública',
  'siteCheck.emptyListsOnePage': '{count} listas no muestran nada a los visitantes, en 1 página pública',
  'siteCheck.emptyLists': '{count} listas no muestran nada a los visitantes, en {pages} páginas públicas',
  'siteCheck.privateOne': '1 bloque apunta a elementos o medios privados, en 1 página pública',
  'siteCheck.privateOnePage': '{count} bloques apuntan a elementos o medios privados, en 1 página pública',
  'siteCheck.private': '{count} bloques apuntan a elementos o medios privados, en {pages} páginas públicas',
  'siteCheck.privateNote': 'Los visitantes no ven los elementos ni los medios privados.',
  'siteCheck.outsideMenuOne': '1 página pública no está en el menú',
  'siteCheck.outsideMenu': '{count} páginas públicas no están en el menú',
  'siteCheck.outsideMenuNote': 'A una página fuera del menú los visitantes solo llegan por su enlace.',
};
