/**
 * Plain words for standard terms, and the search that finds a term from them.
 *
 * A beginner looks for "who wrote it", not `dcterms:creator`. `PLAIN_TERMS`
 * holds, per common term and per locale, a plain name, a one-line hint and the
 * words people use for it. A term the table does not name falls back to its
 * stored label, local name and comment, so a module's or a local vocabulary's
 * terms are found too.
 *
 * `searchTerms` ranks and cuts: matching every loose word against every
 * comment returns most of the vocabulary, so a match must hit a name, a
 * curated word or the term itself, and a result far below the best is
 * dropped.
 */
import type { Locale, TermLabels, TermText } from "../i18n";

/** What the search reads of a property or a class. */
export interface Termed {
  readonly term: string;
  readonly localName: string;
  readonly label: string | null;
  readonly comment: string | null;
}

/** Name, hint, search words. */
type Plain = readonly [string, string, string];

/**
 * The locales plain words are curated in. They are written by a reader of the
 * language, not machine-translated, so every other locale reads English.
 */
export type CuratedLocale = "en" | "es";

/** The curated locale a UI locale reads plain words in. */
export const curatedLocale = (locale: Locale): CuratedLocale =>
  locale === "es" ? "es" : "en";

type PlainEntry = Readonly<Record<CuratedLocale, Plain>>;

export const PLAIN_TERMS: Readonly<Record<string, PlainEntry>> = {
  "dcterms:title": {
    en: ["Title", "The name you give it", "title name called heading caption"],
    es: ["Título", "El nombre que le das", "título nombre llamado encabezado"],
  },
  "dcterms:alternative": {
    en: [
      "Other title",
      "Another name it is known by",
      "other title alternative nickname also known",
    ],
    es: [
      "Otro título",
      "Otro nombre por el que se conoce",
      "otro título alternativo apodo conocido",
    ],
  },
  "dcterms:creator": {
    en: [
      "Creator",
      "Who made or wrote it",
      "who wrote made author sender writer photographer creator by drew painted artist maker",
    ],
    es: [
      "Autor",
      "Quién lo hizo o lo escribió",
      "quién escribió hizo autor remitente escritor fotógrafo creador pintó artista",
    ],
  },
  "dcterms:contributor": {
    en: [
      "Contributor",
      "Someone else who helped make it",
      "who helped contributor editor translator also",
    ],
    es: [
      "Colaborador",
      "Otra persona que ayudó a hacerlo",
      "quién ayudó colaborador editor traductor",
    ],
  },
  "dcterms:publisher": {
    en: [
      "Publisher",
      "Who published or issued it",
      "who published publisher printed issued press",
    ],
    es: ["Editorial", "Quién lo publicó", "quién publicó editorial imprenta"],
  },
  "dcterms:date": {
    en: ["Date", "A date connected to it", "when date year time born died"],
    es: [
      "Fecha",
      "Una fecha relacionada",
      "cuándo fecha año tiempo nació murió",
    ],
  },
  "dcterms:created": {
    en: [
      "Date created",
      "When it was made or written",
      "when date written made created year taken",
    ],
    es: [
      "Fecha de creación",
      "Cuándo se hizo o se escribió",
      "cuándo fecha escrito hecho creado año tomada",
    ],
  },
  "dcterms:issued": {
    en: [
      "Date published",
      "When it was published",
      "when published issued release year",
    ],
    es: [
      "Fecha de publicación",
      "Cuándo se publicó",
      "cuándo publicado emitido año",
    ],
  },
  "dcterms:description": {
    en: [
      "Description",
      "What it is about, in your words",
      "what about description summary notes story",
    ],
    es: [
      "Descripción",
      "De qué trata, con tus palabras",
      "qué trata descripción resumen notas historia",
    ],
  },
  "dcterms:abstract": {
    en: ["Summary", "A short summary", "summary abstract short overview"],
    es: ["Resumen", "Un resumen breve", "resumen breve sinopsis"],
  },
  "dcterms:subject": {
    en: [
      "Subject",
      "Topics it is about",
      "what about topic subject theme keyword tag who is in it",
    ],
    es: [
      "Tema",
      "Los temas que trata",
      "qué trata tema asunto palabra clave etiqueta quién aparece",
    ],
  },
  "dcterms:spatial": {
    en: [
      "Place",
      "Where it is about or was made",
      "where place location town county country address",
    ],
    es: [
      "Lugar",
      "Dónde ocurre o dónde se hizo",
      "dónde lugar sitio pueblo ciudad país dirección",
    ],
  },
  "dcterms:temporal": {
    en: [
      "Time period",
      "The years or era it is about",
      "when period era decade century years",
    ],
    es: [
      "Época",
      "Los años o la época que trata",
      "cuándo época era década siglo años",
    ],
  },
  "dcterms:coverage": {
    en: [
      "Coverage",
      "The place or time it covers",
      "where when coverage covers range",
    ],
    es: [
      "Cobertura",
      "El lugar o el tiempo que abarca",
      "dónde cuándo cobertura abarca",
    ],
  },
  "dcterms:language": {
    en: [
      "Language",
      "The language it is in",
      "language irish english spanish written in",
    ],
    es: [
      "Idioma",
      "El idioma en que está",
      "idioma lengua inglés español escrito en",
    ],
  },
  "dcterms:extent": {
    en: [
      "Size",
      "How many pages, how big",
      "how big size pages length extent duration",
    ],
    es: [
      "Tamaño",
      "Cuántas páginas, qué tamaño",
      "tamaño páginas longitud extensión duración",
    ],
  },
  "dcterms:format": {
    en: [
      "Format",
      "Its file type or physical form",
      "format file type paper print digital",
    ],
    es: [
      "Formato",
      "Su tipo de archivo o forma física",
      "formato archivo tipo papel impreso digital",
    ],
  },
  "dcterms:medium": {
    en: [
      "Material",
      "What it is made of",
      "material medium made of paper ink canvas",
    ],
    es: [
      "Material",
      "De qué está hecho",
      "material soporte hecho papel tinta lienzo",
    ],
  },
  "dcterms:type": {
    en: ["Kind", "What kind of thing it is", "kind type genre sort category"],
    es: ["Clase", "Qué clase de cosa es", "clase tipo género categoría"],
  },
  "dcterms:identifier": {
    en: [
      "Reference number",
      "Your own number or code for it",
      "number code id identifier reference box folder",
    ],
    es: [
      "Número de referencia",
      "Tu propio número o código",
      "número código identificador referencia caja carpeta",
    ],
  },
  "dcterms:source": {
    en: [
      "Source",
      "What it was taken or copied from",
      "source copied from taken original",
    ],
    es: [
      "Fuente",
      "De dónde se tomó o se copió",
      "fuente copiado tomado original",
    ],
  },
  "dcterms:provenance": {
    en: [
      "Where it came from",
      "Who owned it before you",
      "where came from owner provenance history donated found",
    ],
    es: [
      "Procedencia",
      "Quién lo tuvo antes que tú",
      "procedencia dueño historia donado encontrado",
    ],
  },
  "dcterms:rights": {
    en: [
      "Rights",
      "Who may copy or share it",
      "copyright rights permission share license",
    ],
    es: [
      "Derechos",
      "Quién puede copiarlo o compartirlo",
      "derechos autor permiso compartir licencia",
    ],
  },
  "dcterms:rightsHolder": {
    en: [
      "Rights holder",
      "Who owns the rights to it",
      "who owns rights holder copyright owner",
    ],
    es: [
      "Titular de derechos",
      "Quién tiene los derechos",
      "quién titular derechos dueño",
    ],
  },
  "dcterms:license": {
    en: [
      "Licence",
      "The licence others may use it under",
      "licence license creative commons reuse",
    ],
    es: [
      "Licencia",
      "La licencia de uso",
      "licencia creative commons reutilizar",
    ],
  },
  "dcterms:relation": {
    en: [
      "Related to",
      "Something connected to it",
      "related relation connected link see also",
    ],
    es: [
      "Relacionado con",
      "Algo conectado con esto",
      "relacionado relación conectado enlace",
    ],
  },
  "dcterms:isPartOf": {
    en: [
      "Part of",
      "The larger thing it belongs to",
      "part of belongs collection series album box",
    ],
    es: [
      "Parte de",
      "La obra mayor a la que pertenece",
      "parte pertenece colección serie álbum caja",
    ],
  },
  "dcterms:hasPart": {
    en: [
      "Has part",
      "A smaller piece inside it",
      "has part contains pieces pages",
    ],
    es: [
      "Contiene",
      "Una parte más pequeña dentro",
      "contiene partes piezas páginas",
    ],
  },
  "dcterms:references": {
    en: [
      "Mentions",
      "Something it refers to",
      "mentions refers cites references",
    ],
    es: ["Menciona", "Algo a lo que se refiere", "menciona refiere cita"],
  },
  "dcterms:bibliographicCitation": {
    en: ["Citation", "How to cite it", "citation cite reference bibliography"],
    es: ["Cita", "Cómo citarlo", "cita citar referencia bibliografía"],
  },
  "dcterms:audience": {
    en: [
      "Audience",
      "Who it is meant for",
      "audience for whom readers intended",
    ],
    es: ["Público", "Para quién es", "público para quién lectores"],
  },
  "dcterms:accessRights": {
    en: ["Access", "Who may see it", "access who may see restricted private"],
    es: [
      "Acceso",
      "Quién puede verlo",
      "acceso quién puede ver restringido privado",
    ],
  },
  "bibo:recipient": {
    en: [
      "Recipient",
      "Who it was sent or given to",
      "who received recipient sent to addressee given to for",
    ],
    es: [
      "Destinatario",
      "A quién se envió o se dio",
      "quién recibió destinatario enviado dado",
    ],
  },
  "bibo:numPages": {
    en: [
      "Number of pages",
      "How many pages it has",
      "how many pages number count",
    ],
    es: [
      "Número de páginas",
      "Cuántas páginas tiene",
      "cuántas páginas número",
    ],
  },
  "bibo:pages": {
    en: ["Pages", "Which pages it covers", "pages page range"],
    es: ["Páginas", "Qué páginas abarca", "páginas rango"],
  },
  "bibo:volume": {
    en: ["Volume", "The volume number", "volume number book"],
    es: ["Volumen", "El número de volumen", "volumen número tomo"],
  },
  "bibo:edition": {
    en: ["Edition", "Which edition it is", "edition printing version"],
    es: ["Edición", "Qué edición es", "edición impresión versión"],
  },
  "foaf:name": {
    en: ["Name", "A person’s full name", "name person who called full"],
    es: [
      "Nombre",
      "El nombre completo de una persona",
      "nombre persona quién llamado completo",
    ],
  },
  "foaf:givenName": {
    en: [
      "First name",
      "A person’s first name",
      "first given name christian forename",
    ],
    es: [
      "Nombre de pila",
      "El nombre de pila de una persona",
      "nombre pila primer",
    ],
  },
  "foaf:familyName": {
    en: [
      "Surname",
      "A person’s family name",
      "surname family last name maiden",
    ],
    es: ["Apellido", "El apellido de una persona", "apellido familia soltera"],
  },
  "foaf:nick": {
    en: [
      "Nickname",
      "What people called them",
      "nickname called known as pet name",
    ],
    es: ["Apodo", "Cómo le llamaban", "apodo llamado conocido mote"],
  },
  "foaf:birthday": {
    en: ["Birthday", "The day they were born", "birthday born birth date"],
    es: [
      "Cumpleaños",
      "El día en que nació",
      "cumpleaños nació nacimiento fecha",
    ],
  },
  "foaf:based_near": {
    en: ["Lived near", "Where they lived", "where lived home based near town"],
    es: ["Vivía cerca de", "Dónde vivía", "dónde vivía casa cerca pueblo"],
  },
  "foaf:homepage": {
    en: ["Web page", "A web page about them", "web page homepage website link"],
    es: ["Página web", "Una página web sobre esto", "página web sitio enlace"],
  },
  "dctype:StillImage": {
    en: [
      "Photograph or picture",
      "A still image",
      "photo photograph picture image still snapshot",
    ],
    es: [
      "Fotografía o imagen",
      "Una imagen fija",
      "foto fotografía imagen retrato",
    ],
  },
  "dctype:Image": {
    en: ["Image", "Any picture", "image picture visual drawing"],
    es: ["Imagen", "Cualquier imagen", "imagen dibujo visual"],
  },
  "dctype:MovingImage": {
    en: ["Film or video", "Moving pictures", "film video movie footage"],
    es: [
      "Película o vídeo",
      "Imágenes en movimiento",
      "película vídeo filmación",
    ],
  },
  "dctype:Sound": {
    en: [
      "Sound recording",
      "Something you listen to",
      "sound audio recording tape interview music",
    ],
    es: [
      "Grabación de sonido",
      "Algo que se escucha",
      "sonido audio grabación cinta entrevista música",
    ],
  },
  "dctype:Text": {
    en: ["Text", "Something you read", "text written document words"],
    es: ["Texto", "Algo que se lee", "texto escrito documento palabras"],
  },
  "dctype:PhysicalObject": {
    en: [
      "Physical object",
      "A thing you can hold",
      "object thing physical artefact artifact item",
    ],
    es: [
      "Objeto físico",
      "Una cosa que se puede tocar",
      "objeto cosa físico artefacto",
    ],
  },
  "dctype:Collection": {
    en: [
      "Collection",
      "A group of things kept together",
      "collection group set album box",
    ],
    es: [
      "Colección",
      "Un grupo de cosas que van juntas",
      "colección grupo conjunto álbum caja",
    ],
  },
  "dctype:Event": {
    en: [
      "Event",
      "Something that happened",
      "event happened wedding funeral party meeting",
    ],
    es: [
      "Acontecimiento",
      "Algo que ocurrió",
      "acontecimiento evento boda funeral fiesta reunión",
    ],
  },
  "dcterms:Location": {
    en: ["Place", "A place on the map", "place location town country site"],
    es: ["Lugar", "Un lugar en el mapa", "lugar sitio pueblo país"],
  },
  "bibo:Letter": {
    en: [
      "Letter",
      "A written message sent to someone",
      "letter correspondence postcard note mail",
    ],
    es: [
      "Carta",
      "Un mensaje escrito enviado a alguien",
      "carta correspondencia postal nota correo",
    ],
  },
  "bibo:Document": {
    en: [
      "Document",
      "Any written or printed record",
      "document record paper diary journal",
    ],
    es: [
      "Documento",
      "Cualquier registro escrito o impreso",
      "documento registro papel diario",
    ],
  },
  "bibo:Book": {
    en: ["Book", "A bound book", "book volume bound"],
    es: ["Libro", "Un libro encuadernado", "libro tomo encuadernado"],
  },
  "bibo:Article": {
    en: [
      "Article",
      "A piece in a newspaper or journal",
      "article newspaper clipping journal piece",
    ],
    es: [
      "Artículo",
      "Un texto de periódico o revista",
      "artículo periódico recorte revista",
    ],
  },
  "bibo:Map": {
    en: ["Map", "A map or plan", "map plan chart"],
    es: ["Mapa", "Un mapa o plano", "mapa plano carta"],
  },
  "bibo:Manuscript": {
    en: ["Manuscript", "A handwritten text", "manuscript handwritten draft"],
    es: ["Manuscrito", "Un texto escrito a mano", "manuscrito mano borrador"],
  },
  "bibo:Note": {
    en: ["Note", "A short note", "note memo scrap"],
    es: ["Nota", "Una nota breve", "nota apunte"],
  },
  "foaf:Person": {
    en: [
      "Person",
      "Someone, living or dead",
      "person people someone relative family",
    ],
    es: [
      "Persona",
      "Alguien, vivo o muerto",
      "persona gente alguien pariente familia",
    ],
  },
  "foaf:Organization": {
    en: [
      "Organisation",
      "A company, club or group",
      "organisation organization company club society school church",
    ],
    es: [
      "Organización",
      "Una empresa, club o grupo",
      "organización empresa club sociedad escuela iglesia",
    ],
  },
  "foaf:Group": {
    en: ["Group", "A group of people", "group people family band team"],
    es: ["Grupo", "Un grupo de personas", "grupo personas familia equipo"],
  },
};

/** A term's curated plain words in a locale, English where the locale is not curated. */
const inLocale = (entry: PlainEntry, locale: Locale): Plain =>
  entry[curatedLocale(locale)];

const plainOf = (term: string, locale: Locale): Plain | undefined => {
  const entry = PLAIN_TERMS[term];
  return entry === undefined ? undefined : inLocale(entry, locale);
};

/**
 * The plain name in `text`'s locale: the curated one, else the label (a
 * built-in term's translation, any other label as stored), else the term.
 */
export const plainName = (entry: Termed, text: TermLabels): string => {
  const curated = plainOf(entry.term, text.locale)?.[0];
  if (curated !== undefined) {
    return curated;
  }
  const label = text.label(entry.term, entry.label);
  return label === null || label === "" ? entry.term : label;
};

/** The definition in `text`'s locale: a built-in term's translation, any other as stored. */
export const termComment = (entry: Termed, text: TermText): string | null =>
  text.comment(entry.term, entry.comment);

/** The curated one-line hint, if the table names the term. */
export const plainHint = (term: string, locale: Locale): string | null =>
  plainOf(term, locale)?.[1] ?? null;

/**
 * A field's description wherever the admin shows one: the curated hint, else
 * the definition. The Fields page and the form both read it, so one field is
 * never described two ways.
 */
export const fieldDescription = (entry: Termed, text: TermText): string | null =>
  plainHint(entry.term, text.locale) ?? termComment(entry, text);

/**
 * Words a query carries little meaning in. They still count a little, so "who
 * wrote it" prefers the creator over every property whose words hold "wrote".
 */
const WEAK = new Set([
  "a",
  "an",
  "the",
  "it",
  "is",
  "of",
  "to",
  "for",
  "in",
  "on",
  "by",
  "and",
  "or",
  "its",
  "was",
  "this",
  "el",
  "la",
  "los",
  "las",
  "un",
  "una",
  "de",
  "del",
  "en",
  "y",
  "o",
  "es",
  "lo",
  "se",
  "por",
  "para",
]);

const words = (text: string): readonly string[] =>
  text
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => word !== "");

/** A match on a name or the term outweighs a curated word, which outweighs a comment word. */
const NAME = 3;
const CURATED = 2;
const PREFIX = 0.6;
const COMMENT = 0.5;
/** A result must score at least this, and at least this share of the best. */
const FLOOR = 1.5;
const SHARE = 0.4;

const hit = (word: string, pool: readonly string[]): number => {
  if (pool.includes(word)) {
    return 1;
  }
  return word.length >= 3 &&
    pool.some((candidate) => candidate.startsWith(word))
    ? PREFIX
    : 0;
};

const scoreOf = (
  entry: Termed,
  query: readonly string[],
  phrase: string,
  text: TermText,
): number => {
  const curated = PLAIN_TERMS[entry.term];
  const names = words(
    [
      plainName(entry, text),
      entry.label ?? "",
      entry.localName,
      entry.term,
    ].join(" "),
  );
  const pool = words(
    curated === undefined
      ? ""
      : `${inLocale(curated, text.locale)[2]} ${curated.en[2]}`,
  );
  const translated = text.comment(entry.term, entry.comment);
  const comment = words(`${translated ?? ""} ${entry.comment ?? ""}`);
  let score = 0;
  for (const word of query) {
    const weight = WEAK.has(word) ? 0.25 : 1;
    score +=
      weight *
      Math.max(
        NAME * hit(word, names),
        CURATED * hit(word, pool),
        COMMENT * (comment.includes(word) ? 1 : 0),
      );
  }
  const exact = [plainName(entry, text), entry.label ?? "", entry.term].some(
    (name) => name.toLowerCase() === phrase,
  );
  return exact ? score + 2 * NAME : score;
};

/**
 * The entries a plain-words query finds, best first, at most `limit`. An empty
 * query finds nothing.
 */
export const searchTerms = <T extends Termed>(
  entries: readonly T[],
  query: string,
  text: TermText,
  limit = 8,
): readonly T[] => {
  const phrase = query.trim().toLowerCase();
  const tokens = words(phrase);
  if (tokens.length === 0) {
    return [];
  }
  const scored = entries
    .map((entry) => ({ entry, score: scoreOf(entry, tokens, phrase, text) }))
    .filter((row) => row.score >= FLOOR)
    .sort((a, b) => b.score - a.score);
  const best = scored[0]?.score ?? 0;
  return scored
    .filter((row) => row.score >= best * SHARE)
    .slice(0, limit)
    .map((row) => row.entry);
};
