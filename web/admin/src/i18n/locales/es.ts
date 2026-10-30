/**
 * The Spanish catalogue.
 *
 * Spanish is the second locale because upstream ships a Spanish catalogue at
 * this SHA (`source/omeka-s/application/language/es.po`, 1729 message ids, plus
 * `es_419`), so the admin-facing terms have an upstream translation to check
 * against. It is typed `Catalogue`, so a key added to English and not here is a
 * `tsc` error, not a silent English fallback.
 */
import type { Catalogue } from '../catalogue';

export const es: Catalogue = {
  'app.title': 'Administración de {title}',
  'shell.navLabel': 'Páginas de administración',
  'shell.noPage': 'No existe esa página. Vaya al panel de administración.',
  'shell.loadFailed': 'No se pudo cargar esta página. Compruebe la conexión y vuelva a cargarla.',
  'shell.reload': 'Volver a cargar',
  'group.content': 'Contenido',
  'group.admin': 'Administración',
  'page.dashboard': 'Panel de administración',
  'page.items': 'Elementos',
  'page.itemSets': 'Conjuntos de elementos',
  'page.media': 'Medios',
  'page.import': 'Importar',
  'page.sites': 'Sitios',
  'page.fields': 'Campos',
  'page.users': 'Usuarios',
  'page.jobs': 'Tareas',
  'page.settings': 'Configuración',
  'page.assets': 'Recursos',
  'loading.page': 'Cargando la página...',
  'auth.panelHeading': 'Iniciar sesión',
  'auth.gateLead': 'Inicia sesión para gestionar esta colección.',
  'auth.email': 'Correo electrónico',
  'auth.password': 'Contraseña',
  'auth.errorEmpty': 'Introduce tu correo y tu contraseña.',
  'auth.keyDisclosure': 'Usar una clave de API',
  'auth.forgot': '¿Olvidaste tu contraseña?',
  'auth.heading': 'Iniciar sesión con una clave de API',
  'auth.identity': 'Identidad de la clave',
  'auth.credential': 'Credencial de la clave',
  'auth.submit': 'Iniciar sesión',
  'auth.hint': 'Un administrador de la instalación genera una clave con',
  'auth.signedInAs': 'Sesión iniciada como {identity}',
  'auth.signedOut': 'Sesión cerrada',
  'auth.signIn': 'Iniciar sesión',
  'auth.signOut': 'Cerrar sesión',
  'format.httpSuffix': ' (HTTP {status})',
  'format.bytes': '{size} bytes',
  'field.saving': 'Guardando...',
  'locale.label': 'Idioma',
  'locale.en': 'English',
  'locale.es': 'Español',
  'locale.hi': 'हिन्दी',
  'locale.mr': 'मराठी',
  'locale.ml': 'മലയാളം',
  'locale.ta': 'தமிழ்',
  'locale.kn': 'ಕನ್ನಡ',
  'locale.te': 'తెలుగు',
  'locale.ne': 'नेपाली',
  // Term lists and the term picker.
  'page.termLists': 'Listas de términos',
  // The Describe nav group and its Advanced > Standards link.
  'group.describe': 'Describir',
  'nav.advanced': 'Avanzado',
  'page.system': 'Sistema',
  'page.itemTypes': 'Tipos de elemento',
  'page.standards': 'Estándares',
  // The jump-to box in the header and the saved lists under Items.
  'jump.label': 'Ir a un registro',
  'jump.placeholder': '#id, identificador o título',
  'nav.savedLists': 'Listas guardadas',
};
