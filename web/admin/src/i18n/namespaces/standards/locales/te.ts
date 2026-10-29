/**
 * The Telugu Standards and Fields page messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Telugu. Correct
 * a string here; a later run keeps it. It is typed `StandardsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { StandardsMessages } from '../index';

export const te: StandardsMessages = {
  'standards.lede':
    'మీ ఫీల్డ్‌లు మరియు అంశం రకాలను సేవ్ చేసే ఉమ్మడి ప్రమాణాలు. కేటలాగ్ చేయడానికి మీరు వాటిని మార్చాల్సిన అవసరం లేదు.',
  'standards.define': '{term} అంటే ఏమిటి?',
  'standards.import': 'ఒక ప్రమాణాన్ని ఇంపోర్ట్ చేయండి',
  'import.heading': 'ఒక ప్రమాణాన్ని ఇంపోర్ట్ చేయండి',
  'import.lede':
    'RDF ఫైల్ లేదా వెబ్ అడ్రస్ నుండి ప్రచురించబడిన ప్రమాణాన్ని తీసుకురండి. దీని తరగతులు మరియు ప్రాపర్టీలు మీ ఫీల్డ్‌లు మరియు అంశం రకాలకు అందుబాటులో ఉంటాయి.',
  'import.submit': 'ప్రమాణాన్ని ఇంపోర్ట్ చేయండి',
  'import.working': 'ప్రమాణాన్ని చదువుతోంది…',
  'source.legend': 'ప్రమాణం ఎక్కడ నుండి వస్తుంది',
  'source.upload': 'ఈ కంప్యూటర్‌లో ఒక ఫైల్',
  'source.url': 'ఒక వెబ్ అడ్రస్',
  'source.file': 'ప్రమాణ ఫైల్',
  'source.urlLabel': 'ఫైల్ యొక్క వెబ్ అడ్రస్',
  'source.format': 'ఫైల్ ఫార్మాట్',
  'source.formatHelp':
    'ఫైల్ ఫార్మాట్ గురించి ఇంపోర్ట్ చెప్పనంత వరకు, దీనిని “ఆటోమేటిక్‌గా గుర్తించు” (Detect automatically) అని ఉంచండి.',
  'format.guess': 'ఆటోమేటిక్‌గా గుర్తించు',
  'format.rdfxml': 'RDF/XML (.rdf, .owl)',
  'format.turtle': 'ట్రిపుల్ లేదా నోటేషన్3 (.ttl, .n3)',
  'format.ntriples': 'N-ట్రిపుల్స్ (.nt)',
  'format.jsonld': 'JSON-LD (.jsonld)',
  'about.legend': 'ప్రమాణం గురించి',
  'about.namespace': 'నేమ్‌స్పేస్ URI',
  'about.namespaceHelp':
    'ప్రమాణంలోని ప్రతి క్లాస్ మరియు ప్రాపర్టీ ప్రారంభమయ్యే వెబ్ అడ్రస్, ఉదాహరణకు http://xmlns.com/foaf/0.1/. దీని కింద ఉన్న పదాలు మాత్రమే ఇంపోర్ట్ చేయబడతాయి. ఇది సాధారణంగా / లేదా # తో ముగుస్తుంది.',
  'about.prefix': 'ప్రిఫిక్స్',
  'about.prefixHelp':
    'ప్రతి పదానికి ముందు రాసే చిన్న పేరు, ఉదాహరణకు foaf:name. అక్షరాలు, సంఖ్యలు, హైఫన్లు మరియు అండర్ స్కోర్లను ఉపయోగించండి.',
  'about.label': 'లేబుల్',
  'about.labelHelp': 'ఈ ప్రమాణాన్ని ప్రజలు చూసే పేరు.',
  'about.comment': 'వివరణ (ఐచ్ఛికం)',
  'vocabularies.advancedSummary': 'అధునాతన',
  'advanced.lang': 'ప్రాధాన్యత భాష',
  'advanced.langHelp':
    'en లేదా fr వంటి భాషా ట్యాగ్. ఆ భాషలో లేబుల్స్ మరియు కామెంట్లు ఉంచబడతాయి. ఒకటి లేకపోయినా, లేదా ఆ పదంలో ఏదీ లేకపోయినా, ఫైల్‌లో ఉన్న మొదటిది ఉంచబడుతుంది.',
  'advanced.label': 'లేబుల్ ప్రాపర్టీ',
  'advanced.labelHelp':
    'అసాధారణ ప్రాపర్టీతో తన పదాలకు పేరు పెట్టే ప్రమాణం కోసం మాత్రమే. కోణీయ బ్రాకెట్లలో పూర్తి ప్రాపర్టీ URIని వ్రాయండి, ఉదాహరణకు <http://schema.org/name>. దీని లేకుండా, skos:prefLabel, rdfs:label, foaf:name, rss:title మరియు dc:title వరుసగా ప్రయత్నించబడతాయి.',
  'advanced.comment': 'కామెంట్ ప్రాపర్టీ',
  'advanced.commentHelp':
    'ప్రతి పదం యొక్క వివరణ కోసం అదే విధంగా. ఒకటి లేకపోతే, rdfs:comment చదవబడుతుంది.',
  'error.noFile': 'దిగుమతి చేసుకోవడానికి ఒక ఫైల్‌ను ఎంచుకోండి.',
  'error.noUrl': 'ఫైల్ యొక్క వెబ్ అడ్రస్‌ను నమోదు చేయండి.',
  'update.toggle': 'ఫైల్ లేదా వెబ్ అడ్రస్ నుండి అప్‌డేట్ చేయండి',
  'update.lede':
    'ఈ ప్రమాణాన్ని దాని ఫైల్ యొక్క కొత్త కాపీతో పోల్చండి. సేవ్ చేయడానికి ముందు ప్రతి మార్పును మీరు చూడవచ్చు.',
  'update.compare': 'పోల్చండి',
  'update.comparing': 'పోలుస్తున్నాం…',
  'diff.review': 'మీరు వాటిని వర్తింపజేయడానికి ముందు ఈ మార్పులను సమీక్షించండి.',
  'diff.none': 'ఈ ప్రమాణం ఇప్పటికే ఫైల్ తో సరిపోలుతోంది. మార్చడానికి ఏమీ లేదు.',
  'diff.newClasses': 'కొత్త తరగతులు ({count})',
  'diff.newProperties': 'కొత్త లక్షణాలు ({count})',
  'diff.changed': 'మార్చబడిన లేబుల్స్ మరియు వ్యాఖ్యలు ({count})',
  'diff.removed': 'ఇకపై ఫైల్‌లో లేవు ({count})',
  'diff.removedNote':
    'ఇవి ఆర్కైవ్‌లో ఉంటాయి, అలాగే వాటిని ఉపయోగించే విలువల మరియు ఐటమ్ రకాలను కూడా ఉంచుతాయి. మీకు ఇకపై అవసరం లేకపోతే దాని స్వంత పేజీ నుండి ఒకదాన్ని తొలగించండి.',
  'diff.term': 'పదం',
  'diff.label': 'లేబుల్',
  'diff.comment': 'వ్యాఖ్య',
  'diff.what': 'ఏమి మార్పులు',
  'diff.now': 'ఇప్పుడు',
  'diff.file': 'ఫైల్‌లో',
  'diff.blank': '(ఏదీ లేదు)',
  'diff.apply': 'ఈ మార్పులను వర్తింపజేయండి',
  'diff.applying': 'వర్తింపజేయడం జరుగుతోంది…',
  'diff.cancel': 'రద్దు చేయండి',
  'diff.applied': 'మార్పులు సేవ్ చేయబడ్డాయి.',
  'page.properties': 'లక్షణాలు',
  'loading.vocabulary': 'ప్రమాణాన్ని లోడ్ చేస్తున్నాం...',
  'loading.property': 'ప్రాపర్టీ లోడ్ అవుతోంది...',
  'loading.resourceClass': 'క్లాస్‌ను లోడ్ చేస్తున్నాం...',
  'loading.vocabularyForm': 'ప్రమాణ ఫారమ్‌ను లోడ్ చేస్తున్నాం...',
  'loading.propertyForm': 'ప్రాపర్టీ ఫారమ్ లోడ్ అవుతోంది...',
  'loading.resourceClassForm': 'క్లాస్ ఫారమ్‌ను లోడ్ చేస్తున్నాం...',
  'column.comment': 'వ్యాఖ్య',
  'column.term': 'పదం',
  'column.prefix': 'ప్రిఫిక్స్',
  'column.namespace': 'నేమ్‌స్పేస్',
  'column.vocabulary': 'ప్రమాణం',
  'column.properties': 'లక్షణాలు',
  'resource.properties': 'లక్షణాలు',
  'resource.resourceClasses': 'క్లాస్‌లు',
  'resource.vocabularies': 'ప్రమాణాలు',
  'detail.namespaceUri': 'నేమ్‌స్పేస్ URI',
  'detail.localName': 'స్థానిక పేరు',
  'detail.classes': 'తరగతులు',
  'detail.noProperties': 'ఈ ప్రమాణానికి ఎటువంటి లక్షణాలు లేవు.',
  'detail.noClasses': 'ఈ ప్రమాణానికి ఎటువంటి తరగతులు లేవు.',
  'vocabularies.add': 'ప్రమాణాన్ని జోడించండి',
  'vocabularies.legend': 'ప్రమాణం',
  'vocabularies.editLabel': 'ఈ ప్రమాణాన్ని సవరించండి',
  'vocabularies.deleteLabel': 'ఈ ప్రమాణాన్ని తొలగించండి',
  'vocabularies.permanent':
    'Dublin Core మరియు Dublin Core Type సాఫ్ట్‌వేర్‌లో భాగం, కాబట్టి ఈ ప్రమాణాన్ని సవరించలేము లేదా తొలగించలేము.',
  'vocabularies.errorEmptyNamespace': 'నేమ్‌స్పేస్ URI ఖాళీగా ఉండకూడదు.',
  'vocabularies.errorEmptyPrefix': 'ప్రిఫిక్స్ ఖాళీగా ఉండకూడదు.',
  'vocabularies.errorPrefixPattern':
    'ప్రిఫిక్స్ కేవలం అక్షరాలు, సంఖ్యలు, హైఫన్‌లు మరియు అండర్‌స్కోర్‌లను మాత్రమే కలిగి ఉండవచ్చు.',
  'member.legend': 'సభ్యుడు',
  'member.selectVocabulary': 'ఒక ప్రమాణాన్ని ఎంచుకోండి',
  'member.errorEmptyLocalName': 'స్థానిక పేరు ఖాళీగా ఉండకూడదు.',
  'member.errorEmptyVocabulary': 'ఒక ప్రమాణాన్ని తప్పనిసరిగా సెట్ చేయాలి.',
  'properties.add': 'ప్రాపర్టీని జోడించండి',
  'properties.editLabel': 'ఈ ప్రాపర్టీని ఎడిట్ చేయండి',
  'properties.deleteLabel': 'ఈ ప్రాపర్టీని డిలీట్ చేయండి',
  'resourceClasses.add': 'తరగతిని జోడించండి',
  'resourceClasses.editLabel': 'ఈ తరగతిని సవరించండి',
  'resourceClasses.deleteLabel': 'ఈ తరగతిని తొలగించండి',
  'route.invalidVocabularyId': '"{param}" అనేది సరైన ప్రమాణం ఐడి కాదు.',
  'route.invalidPropertyId': '"{param}" అనేది సరైన ప్రాపర్టీ ఐడి కాదు.',
  'route.invalidClassId': '"{param}" అనేది సరైన తరగతి ఐడి కాదు.',
};
