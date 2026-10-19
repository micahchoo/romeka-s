/**
 * The Spanish lessons messages, written by hand.
 *
 * It is typed `LessonsMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { LessonsMessages } from '../index';

export const es: LessonsMessages = {
  'lesson.label': 'Lección: {title}',
  'lesson.step': 'Paso {number} de {count}: {rung}',
  'lesson.gotIt': 'Entendido',
  'lesson.recall': 'Mostrar la lección: {title}',
  'lesson.chip': 'Lección',
  'lesson.learnMore': 'Leer sobre {standard}',
  'rung.describe': 'Describir',
  'rung.kinds': 'Tipos de cosas',
  'rung.fixedValues': 'Valores fijos',
  'rung.linkedData': 'Datos enlazados',
  'rung.makingStandards': 'Crear estándares',
  'standard.dublinCore': 'Dublin Core',
  'standard.rdfClasses': 'las clases de RDF Schema',
  'standard.linkedData': 'los datos enlazados',
  'standard.localVocabulary': 'los vocabularios en Omeka S',
  'standard.skos': 'los vocabularios controlados en SKOS',
  'standard.skosMapping': 'la correspondencia de términos entre vocabularios en SKOS',
  'standard.coolUris': 'las direcciones web que no cambian',
  'term.title': 'Términos estándar',
  'term.body':
    'El código bajo cada campo, como dcterms:creator, es su término estándar de Dublin Core. Los valores se guardan con ese término, así que otro archivo que lea Dublin Core sabe qué es cada valor sin preguntarte.',
  'rename.title': 'Nombres y términos',
  'rename.body':
    'Has cambiado el nombre que la gente ve en el formulario. El valor se sigue guardando con el mismo término estándar, así que la búsqueda, las exportaciones y otros archivos lo leen como antes.',
  'fieldsPage.title': 'Campos compartidos',
  'fieldsPage.body':
    'Cada campo de esta lista es un término de un vocabulario compartido, casi siempre Dublin Core. Cuando dos archivos guardan un creador como dcterms:creator, sus registros se pueden buscar y unir.',
  'required.title': 'Campos obligatorios',
  'required.body':
    'Este tipo de elemento marca el campo como obligatorio, así que cada elemento de este tipo tiene un valor aquí. Una colección en la que todos los registros lo tienen se puede ordenar y buscar sin huecos.',
  'typeClass.title': 'Clases estándar',
  'typeClass.body':
    'El nombre es para las personas. La clase estándar, como bibo:Letter, dice al software qué clase de cosa es cada elemento, así que una búsqueda de esa clase, aquí o en otro archivo, encuentra todos los elementos de este tipo.',
  'typePick.title': 'Tipos de elemento',
  'typePick.body':
    'Un tipo organiza los campos de una clase de cosa y guarda el elemento con su clase estándar. Los elementos de un mismo tipo comparten los mismos campos, así que es fácil compararlos y encontrarlos juntos.',
  'standards.title': 'Datos enlazados',
  'standards.body':
    'Estos son los vocabularios detrás de cada campo y cada clase. Cada término tiene una dirección web, como http://purl.org/dc/terms/title, así que un valor guardado aquí significa lo mismo dondequiera que se lea.',
  'localField.title': 'Tu propio vocabulario',
  'localField.body':
    'Su nombre empieza por local: porque ningún estándar compartido tenía este campo, así que es un término del vocabulario propio de este archivo. Su dirección está en este sitio, así que otros archivos pueden leer el valor y consultar qué significa.',
  'termMerge.title': 'Una sola grafía',
  'termMerge.body':
    'Al fusionar, cada valor escrito de otra forma pasa a la grafía que conservas. Con una sola grafía para cada cosa, una búsqueda o una lista de valores encuentra todos los registros, no solo los que se escribieron igual.',
  'tagPromote.title': 'Un vocabulario controlado',
  'tagPromote.body':
    'Al pasarlas a una lista, las etiquetas elegidas son las palabras de esa lista, y el campo Tema de estos tipos de elemento ofrece la lista. Las personas eligen una palabra en lugar de escribirla, así que cada cosa conserva una sola grafía.',
  'authorityLink.title': 'Enlaces a una autoridad',
  'authorityLink.body':
    'Una coincidencia exacta dice que este término y un registro de una autoridad, como GeoNames o Wikidata, nombran la misma cosa. La lista publicada lleva el enlace como skos:exactMatch, así que el software que conoce ese registro puede unir tus registros con todo lo que está enlazado a él.',
  'publish.title': 'Direcciones que duran',
  'publish.body':
    'Al publicar, cada término recibe una dirección web que nunca cambia, y las versiones siguientes pueden añadir términos, pero nunca quitar uno. Los registros de este y de otros archivos pueden citar esas direcciones y confiar en que los enlaces siguen funcionando.',
  'switch.label': 'Mostrar lecciones',
  'switch.hint':
    'Notas breves que explican el estándar detrás de una acción, la primera vez que la haces.',
};
