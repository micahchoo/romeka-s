/**
 * The Spanish Standards and Fields page messages, written by hand.
 *
 * It is typed `StandardsMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { StandardsMessages } from '../index';

export const es: StandardsMessages = {
  'standards.lede':
    'Los vocabularios compartidos en los que se guardan sus campos y tipos de elemento. No necesita cambiarlos para catalogar una colección.',
  'standards.define': 'Qué significa {term}',
  'standards.import': 'Importar un vocabulario',
  'import.heading': 'Importar un vocabulario',
  'import.lede':
    'Traiga un vocabulario publicado desde un archivo RDF o una dirección web. Sus clases y propiedades quedan disponibles para sus campos y tipos de elemento.',
  'import.submit': 'Importar vocabulario',
  'import.working': 'Leyendo el vocabulario…',
  'source.legend': 'De dónde viene el vocabulario',
  'source.upload': 'Un archivo de este equipo',
  'source.url': 'Una dirección web',
  'source.file': 'Archivo de vocabulario',
  'source.urlLabel': 'Dirección web del archivo',
  'source.format': 'Formato de archivo',
  'source.formatHelp':
    'Deje «Detección automática» salvo que la importación diga que no reconoce el formato.',
  'format.guess': 'Detección automática',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'Turtle o Notation3 (.ttl, .n3)',
  'format.ntriples': 'N-Triples (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'Sobre el vocabulario',
  'about.namespace': 'URI del espacio de nombres',
  'about.namespaceHelp':
    'La dirección web con la que empiezan todas las clases y propiedades del vocabulario, como http://xmlns.com/foaf/0.1/. Solo se importan los términos que empiezan así. Suele terminar en / o #.',
  'about.prefix': 'Prefijo',
  'about.prefixHelp':
    'Un nombre corto que se escribe antes de cada término, como en foaf:name. Use letras, números, guiones y guiones bajos.',
  'about.label': 'Etiqueta',
  'about.labelHelp': 'El nombre que se muestra para este vocabulario.',
  'about.comment': 'Descripción (opcional)',
  'vocabularies.advancedSummary': 'Avanzado',
  'advanced.lang': 'Idioma preferido',
  'advanced.langHelp':
    'Una etiqueta de idioma como es o en. Se guardan las etiquetas y los comentarios en ese idioma. Sin ella, o para un término que no lo tiene, se guarda el primero del archivo.',
  'advanced.label': 'Propiedad de etiqueta',
  'advanced.labelHelp':
    'Solo para un vocabulario que nombra sus términos con una propiedad poco habitual. Escriba el URI completo de la propiedad entre ángulos, como <http://schema.org/name>. Sin ella, se prueban por orden skos:prefLabel, rdfs:label, foaf:name, rss:title y dc:title.',
  'advanced.comment': 'Propiedad de comentario',
  'advanced.commentHelp':
    'Lo mismo, para la descripción de cada término. Sin ella, se lee rdfs:comment.',
  'error.noFile': 'Elija un archivo para importar.',
  'error.noUrl': 'Escriba la dirección web del archivo.',
  'update.toggle': 'Actualizar desde un archivo o una dirección web',
  'update.lede':
    'Compare este vocabulario con una copia más reciente de su archivo. Verá cada cambio antes de que se guarde nada.',
  'update.compare': 'Comparar',
  'update.comparing': 'Comparando…',
  'diff.review': 'Revise estos cambios antes de aplicarlos.',
  'diff.none': 'Este vocabulario ya coincide con el archivo. No hay nada que cambiar.',
  'diff.newClasses': 'Clases nuevas ({count})',
  'diff.newProperties': 'Propiedades nuevas ({count})',
  'diff.changed': 'Etiquetas y comentarios cambiados ({count})',
  'diff.removed': 'Ya no están en el archivo ({count})',
  'diff.removedNote':
    'Se conservan en la colección, igual que los valores y los tipos de elemento que los usan. Elimine uno desde su propia página si ya no lo necesita.',
  'diff.term': 'Término',
  'diff.label': 'Etiqueta',
  'diff.comment': 'Comentario',
  'diff.what': 'Qué cambia',
  'diff.now': 'Ahora',
  'diff.file': 'En el archivo',
  'diff.blank': '(ninguno)',
  'diff.apply': 'Aplicar estos cambios',
  'diff.applying': 'Aplicando…',
  'diff.cancel': 'Cancelar',
  'diff.applied': 'Los cambios se han guardado.',
};
