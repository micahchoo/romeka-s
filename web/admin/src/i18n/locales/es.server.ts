/**
 * The Spanish server messages: one entry per pattern in
 * `src/generated/serverMessages.json`, with the same `{name}` holes.
 *
 * The text is upstream's `application/language/es.po` wherever the English
 * pattern is an upstream msgid with a translation (48 of 150); the other
 * 102 are the port's own. It is typed `ServerCatalogue`, so a pattern added
 * on the Rust side and missing here is a `tsc` error. It is its own chunk,
 * fetched the first time an error arrives in Spanish.
 */
import type { ServerCatalogue } from '../serverMessages';

export const esServer: ServerCatalogue = {
  'server.methodNotAllowed': 'Método no permitido',
  'server.notFound': 'No encontrado',
  'server.badRequest': 'Solicitud incorrecta',
  'server.requestTimeout': 'Se agotó el tiempo de espera de la solicitud',
  'server.bodyTooLarge': 'El cuerpo de la solicitud es demasiado grande',
  'server.internalError': 'Error interno del servidor',
  'server.tooManyRequests': 'Demasiadas solicitudes',
  'server.unknownResource': 'La API no soporta el recurso "{resource}".',
  'server.adapterNotImplemented': 'El adaptador {resource} no implementa la operación {operation}.',
  'server.endpointNotImplemented': 'El punto de acceso {endpoint} aún no está implementado.',
  'server.permissionDenied': 'Permiso denegado al usuario actual para que acceda a {operation} del recurso {resource}.',
  'server.entityNotFound': 'No se ha encontrado la entidad {class} con el criterio {criteria}',
  'server.jsonSyntax': 'JSON: error de sintaxis',
  'server.jsonNotContainer': 'JSON: el contenido debe ser un objeto o un vector.',
  'server.invalidContentType': 'Cabecera Content-Type no válida. Se esperaba "application/json" y se recibió "{received}".',
  'server.bodyNotObject': 'El cuerpo de la solicitud debe ser un objeto JSON.',
  'server.siteItemsOneSelector': 'indique exactamente uno de "query", "items" o "item_sets"',
  'server.invalidCredentials': 'Correo electrónico o contraseña no válidos',
  'server.inactiveAccount': 'El usuario no está activo.',
  'server.unknownKey': 'La clave de creación de contraseña no es válida.',
  'server.authenticationRequired': 'Se requiere autenticación.',
  'server.invalidPassword': 'La contraseña es incorrecta',
  'server.crossOriginRefused': 'Una escritura autenticada por sesión debe venir de la URL base configurada.',
  'server.propertyNotRecognized': 'No se reconoce la propiedad "{term}".',
  'server.resourceNotFound': 'El recurso con identidad {id} no se ha encontrado.',
  'server.templateRequiresValue': 'La plantilla de recursos "{template}" requiere un valor "{property}"',
  'server.itemSetsNotArray': 'Los conjuntos de fichas deben almacenarse en un vector o "array"',
  'server.mediaNotArray': 'Los medios deben almacenarse en un vector o "array"',
  'server.mediaNotThisItem': 'Los medios deben pertenecer a esta ficha.',
  'server.primaryMediaNotThisItem': 'El medio principal debe pertenecer a esta ficha.',
  'server.mediaIngesterRequired': 'Los medios deben tener definida una forma de incorporación.',
  'server.mediaIngesterInvalid': 'Los medios deben incorporarse a través de una forma válida.',
  'server.mediaItemRequired': 'Los medios deben pertenecer a una ficha.',
  'server.jobHeldByImport':
    'Esta tarea pertenece a una importación de archivo que se está ejecutando o deshaciendo. Elimínala cuando la importación haya terminado.',
  'server.noFileUploaded': 'No se ha subido ningún archivo',
  'server.assetMediaType': 'Error validando "{file}". No se pueden almacenar archivos con el formato "{media}".',
  'server.assetExtension': 'Error validando "{file}". No se pueden almacenar archivos con la extensión "{extension}".',
  'server.assetNameRequired': 'Un recurso debe tener un nombre.',
  'server.classesNotArray': 'Las clases deben almacenarse en un vector o "array"',
  'server.propertiesNotArray': 'Las propiedades deben almacenarse en un vector o "array"',
  'server.namespaceUriEmpty': 'El URI del espacio de nombres no puede estar vacío.',
  'server.namespaceUriTaken': 'El URI del espacio de nombres "{uri}" ya está en uso.',
  'server.prefixEmpty': 'El prefijo no puede estar vacío.',
  'server.prefixInvalid': 'El prefijo solo puede contener letras, números, guiones y guiones bajos.',
  'server.prefixTaken': 'El prefijo "{prefix}" ya está en uso.',
  'server.prefixReserved': 'El prefijo "{prefix}" está reservado.',
  'server.labelEmpty': 'La etiqueta no puede estar vacía.',
  'server.labelTaken': 'La etiqueta ya está en uso',
  'server.localNameEmpty': 'El nombre local no puede estar vacío.',
  'server.localNameTaken': 'El nombre local "{name}" ya está en uso.',
  'server.vocabularyRequired': 'Se debe establecer un vocabulario.',
  'server.duplicateTemplateProperty': 'Intentando añadir propiedad duplicada con ID {id}',
  'server.vocabularyNoFile': 'No se ha indicado ningún archivo para importar.',
  'server.vocabularyNoUrl': 'No se ha indicado ninguna URL para importar.',
  'server.vocabularyNotParsed': 'No se pudo leer el archivo de vocabulario.',
  'server.vocabularyNotLoaded': 'No se pudo cargar el vocabulario desde la URL.',
  'server.vocabularyNoMembers':
    'No se encontraron clases ni propiedades. ¿Seguro que el URI del espacio de nombres es correcto?',
  'server.vocabularyPermanent': 'No se puede editar un vocabulario permanente',
  'server.vocabularyFormatUnknown':
    'No se pudo detectar el formato del archivo. Elija el formato en la lista.',
  'server.vocabularyFormatInvalid': '"{format}" no es un formato de archivo de vocabulario.',
  'server.vocabularyUrlScheme': 'Solo se pueden importar URL http y https.',
  'server.vocabularyUrlPrivate':
    'La URL apunta a una dirección de red privada o local, que el servidor no consulta.',
  'server.vocabularyUrlTooLarge': 'El archivo de la URL supera {size}.',
  'server.vocabularyUrlTimeout': 'El servidor de la URL no respondió a tiempo.',
  'server.vocabularyUrlStatus': 'El servidor de la URL respondió con el estado {status}.',
  'server.vocabularyChangeStale':
    '"{name}" ha cambiado desde la comparación. Vuelva a comparar el archivo.',
  'server.vocabularyMemberMissing': '"{name}" no pertenece a este vocabulario.',
  'server.nameEmpty': 'El nombre no puede estar vacío.',
  'server.emailTaken': 'El correo electrónico {email} ya está en uso.',
  'server.userRoleRequired': 'Los perfiles deben tener asignado un rol.',
  'server.invalidEmail': 'La entrada no es una dirección de correo electrónico válida. Use el formato básico parte-local@nombre-de-host',
  'server.siteTitleRequired': 'Un sitio debe tener un título.',
  'server.siteThemeRequired': 'Un sitio debe tener un tema.',
  'server.siteItemPoolRequired': 'Un sitio debe tener asignadas fichas.',
  'server.homepageNotInSite': 'Una página de inicio debe pertenecer a su sitio principal.',
  'server.slugEmpty': 'La ruta no puede estar vacía.',
  'server.slugInvalid': 'Una ruta sólo puede contener letras, números, guiones y guiones bajos.',
  'server.slugTaken': 'La ruta "{slug}" ya está en uso.',
  'server.pageSiteRequired': 'Una página debe pertenecer a un sitio.',
  'server.pageTitleRequired': 'Una página debe tener un título.',
  'server.blockLayoutRequired': 'Todos los bloques deben tener un diseño.',
  'server.blockDataScalar': 'Los datos del bloque no deben ser valores escalables.',
  'server.siteSettingsNotObject': 'Se esperaba un objeto o null, se recibió {kind}.',
  'server.cssTooLarge': 'CSS propio: línea {line}, columna {column}: el CSS propio puede ocupar como máximo {max} bytes; este ocupa {bytes}.',
  'server.cssImport': 'CSS propio: línea {line}, columna {column}: @import no está permitido.',
  'server.cssDocument': 'CSS propio: línea {line}, columna {column}: @document no está permitido.',
  'server.cssForeignUrl': 'CSS propio: línea {line}, columna {column}: url({url}) no está permitida; una URL debe ser una ruta dentro de /files/ o /assets/.',
  'server.cssExpression': 'CSS propio: línea {line}, columna {column}: expression() no está permitido.',
  'server.cssBehavior': 'CSS propio: línea {line}, columna {column}: behavior no está permitido.',
  'server.cssMozBinding': 'CSS propio: línea {line}, columna {column}: -moz-binding no está permitido.',
  'server.cssStyleBreakout': 'CSS propio: línea {line}, columna {column}: </style no está permitido.',
  'server.cssBadUrl': 'CSS propio: línea {line}, columna {column}: este url( no es CSS válido.',
  'server.cssUnclosedString': 'CSS propio: línea {line}, columna {column}: esta cadena no está cerrada.',
  'server.cssStrayClose': 'CSS propio: línea {line}, columna {column}: {bracket} no cierra nada.',
  'server.navNotArray': 'Navegación no válida: debe almacenarse en un vector o "array"',
  'server.navLinkMissingType': 'Navegación no válida: falta el tipo de enlace',
  'server.navLinkMissingData': 'Navegación no válida: faltan datos del enlace',
  'server.navInvalidLinkData': 'Navegación no válida: datos de enlace no válidos',
  'server.navPageLinksNotUnique': 'Navegación no válida: los enlaces de las páginas deben ser únicos',
  'server.navLinksNotArray': 'Navegación no válida: los enlaces deben almacenarse en un vector o "array"',
  'server.navPageLinkMissingPage': 'Navegación no válida: al enlace de página le falta el ID de la página',
  'server.navUrlMissingLabel': 'Navegación no válida: al enlace URL le falta la etiqueta',
  'server.navUrlMissingUrl': 'Navegación no válida: al enlace URL le falta la URL',
  'server.navUrlInvalidCharacter': 'Navegación no válida: el enlace URL contiene un carácter no válido',
  'server.navUrlInvalidScheme': 'Navegación no válida: el enlace URL tiene un esquema no válido',
  'server.settingsDenied': 'Permiso denegado al usuario actual para que acceda a la configuración global.',
  'server.settingsNotObject': 'Se esperaba un objeto.',
  'server.settingUnavailable': 'El ajuste {key} no está disponible.',
  'server.settingNotText': 'El ajuste {key} debe ser una cadena o null.',
  'server.settingNotFlag': 'El ajuste {key} debe ser true, false o null.',
  'server.settingNotNames': 'El ajuste {key} debe ser una lista de cadenas o null.',
  'server.publicationTermsRemoved':
    'Un término publicado no se puede quitar ni renombrar: {terms}. Márcalo como obsoleto.',
  'server.publicationNamespaceFixed':
    'Un vocabulario publicado conserva su espacio de nombres, así que sus términos conservan sus URI.',
  'server.publicationNotDeletable':
    'Una lista de términos o un vocabulario publicados no se pueden borrar. Marca sus términos como obsoletos.',
  'server.publicationDeprecationFixed':
    'La marca de obsoleto de "{term}" está publicada y no se puede retirar.',
  'server.publicationItemsList':
    'Una lista de ítems no se puede publicar. Cada ítem ya tiene su propia dirección.',
  'server.publicationForeignNamespace':
    'El espacio de nombres {namespace} no es de este archivo. Publica un vocabulario cuyo espacio de nombres esté bajo la dirección /vocab/ de este archivo, o en w3id.org o purl.org.',
  'server.publicationSlugInvalid':
    'El nombre de una dirección usa minúsculas, dígitos y guiones sueltos, hasta 64 caracteres.',
  'server.publicationSlugTaken': 'La dirección /vocab/{slug}/ ya está ocupada.',
  'server.publicationNamespaceNotPersistent':
    'Un espacio de nombres que no es el de este archivo debe estar en w3id.org o purl.org y terminar con / o #.',
  'server.publicationAddressFixed':
    'Una lista de términos o un vocabulario publicados conservan la dirección de su primera publicación.',
  'server.publicationTermNotPublished':
    '"{term}" aún no está publicado, así que no se puede marcar como obsoleto. Quítalo.',
  'server.publicationUnknownTerm': '"{term}" no es un término de esta lista o vocabulario.',
  'server.publicationMatchInvalid': 'Un enlace debe ser un URI http o https absoluto.',
  'server.publicationMatchTermLists':
    'Solo los términos de una lista de términos se enlazan a una autoridad.',
  'server.publicationTermIriInvalid': 'El URI de término {uri} no es un IRI válido.',
  'server.importAdminsOnly': 'Solo un administrador global o de sitio puede importar un archivo.',
  'server.importNotFound': 'Ninguna importación tiene el id {id}.',
  'server.importRequestUnreadable': 'No se puede leer la solicitud: {reason}',
  'server.importPlanRefused': 'No se pueden guardar los ajustes de la importación: {reason}',
  'server.importStale':
    'Otra persona guardó esta importación después de que la abrieras. Vuelve a cargarla para ver sus cambios.',
  'server.importPlanFrozen':
    'Esta importación ya empezó, así que sus ajustes no pueden cambiar. Haz una copia para cambiarlo.',
  'server.importV1Plan':
    'Esta importación se hizo con el importador antiguo, así que no se puede abrir ni copiar aquí.',
  'server.importTypeExists':
    'Ya hay un tipo de ficha llamado "{label}" en Omeka. Elige otro nombre para el tipo nuevo.',
  'server.importDeleteNotDraft':
    'Solo se puede eliminar una importación que aún no se ha añadido. Una importación que se ha ejecutado queda en Importaciones anteriores.',
  'server.importChanged':
    'Los archivos o los ajustes de la importación cambiaron después de la última comprobación. Vuelve a comprobar la importación antes de ejecutarla.',
  'server.importBlocked':
    'Esta importación no puede ejecutarse hasta que se corrijan los problemas que indica la comprobación.',
  'server.importNothingToRun': 'Esta importación no tiene nada que escribir.',
  'server.importConfirmDelete':
    'Esta importación elimina {count} elementos, medios y conjuntos de elementos y no se puede deshacer. Confirma con ese número para ejecutarla.',
  'server.importBusy':
    'Otra importación se está ejecutando o deshaciendo. Vuelve a intentarlo cuando haya terminado.',
  'server.importCannotRun':
    'Solo puede ejecutarse una importación que aún no se ha añadido, que se detuvo antes del final o que terminó con fallos.',
  'server.importCannotUndo':
    'Solo se puede deshacer una importación que terminó o que se detuvo antes del final.',
  'server.importNotRunning': 'Esta importación no se está ejecutando.',
  'server.importNoRoot': 'No hay ninguna carpeta de archivo llamada "{root}".',
  'server.importOutsideRoot': 'La carpeta "{path}" está fuera de la carpeta de archivo.',
  'server.importFolderMissing': 'No hay ninguna carpeta "{path}" en el archivo.',
  'server.importNotAFolder': '"{path}" es un archivo, no una carpeta.',
  'server.importScanBusy':
    'Esta importación se está ejecutando o deshaciendo. Vuelve a mirar los archivos cuando haya terminado.',
  'server.importNoLeftFile':
    'Esta importación aún no tiene una lista de archivos que quedaron fuera. Se escribe cuando termina una ejecución.',
  'server.importJobNotStarted': 'No se pudo iniciar la importación: {reason}',
  'server.importFileUnreadable': 'No se pudo leer el archivo "{path}": {reason}',
  'server.importFileRefused': 'El archivo "{path}" no se añadió: {reason}',
  'server.importFileNotStored': 'No se pudo guardar el archivo "{path}": {reason}',
  'server.importSourceRefused': '"{source}" no se añadió: {reason}',
  'server.importMediaNoSource': 'Este medio no indica ningún archivo ni ninguna dirección que añadir.',
  'server.importMediaNoItem':
    'El elemento de este medio no se añadió, así que el medio tampoco se añadió.',
  'server.importResourceGone':
    'El elemento, el medio o el conjunto de elementos que cambia este registro ya no está en Omeka.',
  'server.importRecordNotSaved': 'No se pudo guardar este registro: {reason}',
  'server.importLinksNotSaved': 'No se pudieron guardar los enlaces de este registro: {reason}',
  'server.importSitesNotJoined':
    'Este elemento se añadió, pero no se unió a sus sitios: {reason}',
};
