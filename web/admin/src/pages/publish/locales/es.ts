/**
 * The Publish panel's Spanish messages, loaded with the panel when Spanish is active.
 */
import type { PublishMessages } from '../messages';

export const es: PublishMessages = {
  'publish.heading': 'Publicar',
  'publish.ledeTermList':
    'Publicar da a esta lista y a cada uno de sus términos una dirección web permanente que pueden leer las personas y los programas.',
  'publish.ledeVocabulary':
    'Publicar da a este vocabulario y a cada uno de sus términos una dirección web permanente que pueden leer las personas y los programas.',
  'publish.loading': 'Cargando la publicación...',
  'publish.stateDraft': 'Borrador. Aún no se ha publicado.',
  'publish.statePublished': 'Publicado. Versión {version}, {date}.',
  'publish.publicUrl': 'Dirección pública',
  'publish.promise':
    'Una vez publicado, se pueden añadir términos o marcarlos como obsoletos, pero nunca quitarlos ni cambiarles el nombre, y la dirección no cambia nunca. Otros archivos pueden enlazarlos ya.',
  'publish.slugLabel': 'Nombre de la dirección',
  'publish.slugHint': 'Minúsculas, dígitos y guiones sueltos. Se sirve en {url}',
  'publish.slugInvalid': 'Usa minúsculas, dígitos y guiones sueltos, hasta 64 caracteres.',
  'publish.namespaceLegend': 'Direcciones de los términos',
  'publish.namespaceArchive': 'La dirección de este archivo: {url}',
  'publish.namespacePersistent': 'Una dirección persistente en w3id.org o purl.org',
  'publish.namespaceWhy':
    'Una dirección persistente sobrevive a un cambio de dominio: el servicio la redirige aquí, así que los enlaces a tus términos siguen funcionando si este archivo se muda.',
  'publish.namespaceLabel': 'Espacio de nombres persistente',
  'publish.namespaceHint': 'Por ejemplo https://w3id.org/tu-archivo/lugares/. Termínalo con / o #.',
  'publish.namespaceInvalid': 'Usa una dirección en w3id.org o purl.org que termine con / o #.',
  'publish.redirectHelp': 'Después pide al servicio que la redirija aquí, por ejemplo con esta regla de w3id.org:',
  'publish.namespace': 'Espacio de nombres',
  'publish.termsHeading': 'Términos',
  'publish.columnTerm': 'Término',
  'publish.columnUri': 'URI',
  'publish.columnState': 'Estado',
  'publish.columnChange': 'Cambiar',
  'publish.termDraft': 'Aún no publicado',
  'publish.termPublished': 'Publicado',
  'publish.termDeprecated': 'Obsoleto',
  'publish.termDeprecationPending': 'Obsoleto desde la próxima versión',
  'publish.deprecate': 'Marcar como obsoleto',
  'publish.deprecateLabel': 'Marcar {term} como obsoleto',
  'publish.undeprecate': 'Mantener',
  'publish.undeprecateLabel': 'Mantener {term} en uso',
  'publish.links': 'Enlaces',
  'publish.linksLabel': 'Enlaces de {term} a una autoridad',
  'publish.exactMatch': 'El mismo término en otro sitio (exactMatch)',
  'publish.closeMatch': 'Un término cercano en otro sitio (closeMatch)',
  'publish.linksHint': 'La dirección de este término en una autoridad como GeoNames o Wikidata.',
  'publish.saveLinks': 'Guardar enlaces',
  'publish.matchInvalid': 'Usa una dirección que empiece por http:// o https://.',
  'publish.publish': 'Publicar',
  'publish.publishVersion': 'Publicar la versión {version}',
  'publish.noChanges': 'Nada ha cambiado desde la versión {version}.',
  'publish.pending':
    'Los cambios desde la versión {version} aún no son públicos. Publica para añadirlos.',
  'publish.published': 'Versión {version} publicada.',
  'publish.versionsHeading': 'Versiones',
  'publish.versionLine': 'Versión {version}, {date}, {terms} términos',
  'publish.previewHeading': 'Vista previa en Turtle',
};
