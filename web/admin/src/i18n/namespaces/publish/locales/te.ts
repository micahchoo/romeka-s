/**
 * The Telugu Publish panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Telugu. Correct
 * a string here; a later run keeps it. It is typed `PublishMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PublishMessages } from '../index';

export const te: PublishMessages = {
  'publish.heading': 'ప్రచురించండి',
  'publish.ledeTermList':
    'ప్రచురణ ఈ జాబితాను మరియు దానిలోని ప్రతి పదాన్ని శాశ్వత వెబ్ చిరునామాను ఇస్తుంది, దీనిని ప్రజలు మరియు ప్రోగ్రామ్‌లు చదవవచ్చు.',
  'publish.ledeVocabulary':
    'ప్రచురించడం వల్ల ఈ ప్రమాణం మరియు దాని ప్రతి నిబంధనకు శాశ్వత వెబ్ అడ్రస్ లభిస్తుంది, దీనిని ప్రజలు మరియు ప్రోగ్రామ్‌లు చదవవచ్చు.',
  'publish.loading': 'ప్రచురణను లోడ్ చేస్తున్నాము...',
  'publish.stateDraft': 'ముసాయిదా. ఇంకా ప్రచురించబడలేదు.',
  'publish.statePublished': 'ప్రచురించబడింది. వెర్షన్ {version}, {date}.',
  'publish.publicUrl': 'బహిరంగ చిరునామా',
  'publish.promise':
    'ప్రచురించబడిన తర్వాత, పదాలను జోడించవచ్చు లేదా తొలగించవచ్చు, కానీ వాటిని ఎప్పటికీ తొలగించలేరు లేదా పేరు మార్చలేరు, మరియు చిరునామా ఎప్పటికీ మారదు. ఇతర ఆర్కైవ్‌లు ఇప్పటికే వాటికి లింక్ చేసి ఉండవచ్చు.',
  'publish.slugLabel': 'చిరునామా పేరు',
  'publish.slugHint': 'చిన్న అక్షరాలు, అంకెలు మరియు ఒకే ఒక హైఫన్. ఇది {url} వద్ద అందించబడుతుంది',
  'publish.slugInvalid': 'చిన్న అక్షరాలు, అంకెలు మరియు ఒకే ఒక హైఫన్, గరిష్టంగా 64 అక్షరాలు.',
  'publish.namespaceLegend': 'పదాల చిరునామాలు',
  'publish.namespaceArchive': 'ఈ ఆర్కైవ్ చిరునామా: {url}',
  'publish.namespacePersistent': 'w3id.org లేదా purl.org వద్ద శాశ్వత చిరునామా',
  'publish.namespaceWhy':
    'శాశ్వత చిరునామా మరొక డొమైన్‌కు మారినా పనిచేస్తుంది: ఈ సేవ దానిని ఇక్కడకు మళ్లిస్తుంది, కాబట్టి మీ పదాలకు సంబంధించిన లింక్‌లు పనిచేస్తూనే ఉంటాయి.',
  'publish.namespaceLabel': 'శాశ్వత నేమ్‌స్పేస్',
  'publish.namespaceHint':
    'ఉదాహరణకు https://w3id.org/your-archive/places/. దీనిని / లేదా # తో ముగించండి.',
  'publish.namespaceInvalid':
    'w3id.org లేదా purl.org లో / లేదా # తో ముగిసే చిరునామాను ఉపయోగించండి.',
  'publish.redirectHelp': 'అప్పుడు ఈ w3id.org నియమాన్ని ఉపయోగించి ఇక్కడకు మళ్లించమని కోరండి:',
  'publish.namespace': 'నేమ్‌స్పేస్',
  'publish.termsHeading': 'పదాలు',
  'publish.columnTerm': 'పదం',
  'publish.columnUri': 'URI',
  'publish.columnState': 'స్థితి',
  'publish.columnChange': 'మార్పు',
  'publish.termDraft': 'ఇంకా ప్రచురించబడలేదు',
  'publish.termPublished': 'ప్రచురించబడింది',
  'publish.termDeprecated': 'వాడుకలో లేకుండా పోయింది',
  'publish.termDeprecationPending': 'తదుపరి వెర్షన్ నుండి వాడుకలో లేకుండా పోయింది',
  'publish.deprecate': 'వాడుకలో లేనిదిగా గుర్తించండి',
  'publish.deprecateLabel': '{term}ను వాడుకలో లేకుండా పోయేలా చేయండి',
  'publish.undeprecate': 'ఉంచు',
  'publish.undeprecateLabel': '{term} పదాన్ని ఉంచు',
  'publish.links': 'లింకులు',
  'publish.linksLabel': '{term} మరియు ఒక అధికారానికి మధ్య లింకులు',
  'publish.exactMatch': 'అదే పదం మరెక్కడైనా (exactMatch)',
  'publish.closeMatch': 'ఒక సమీప పదం మరెక్కడైనా (closeMatch)',
  'publish.linksHint': 'GeoNames లేదా Wikidata వంటి అధికారంలో ఈ పదం యొక్క చిరునామా.',
  'publish.saveLinks': 'లింకులను సేవ్ చేయండి',
  'publish.matchInvalid': 'http:// లేదా https:// తో ప్రారంభమయ్యే చిరునామాను ఉపయోగించండి.',
  'publish.publish': 'ప్రచురించండి',
  'publish.publishVersion': 'వెర్షన్ {version} ను ప్రచురించండి',
  'publish.noChanges': 'వెర్షన్ {version} నుండి ఎటువంటి మార్పులు లేవు.',
  'publish.pending':
    'వెర్షన్ {version} నుండి వచ్చిన మార్పులు ఇంకా ప్రైవేట్‌గానే ఉన్నాయి. వాటిని ప్రచురించండి.',
  'publish.published': 'వెర్షన్ {version} ప్రచురించబడింది.',
  'publish.versionsHeading': 'సంస్కరణలు',
  'publish.versionLine': 'వెర్షన్ {version}, {date}, {terms} నిబంధనలు',
  'publish.previewHeading': 'Turtle లో ప్రివ్యూ',
  'publish.replacedBy': '“{term}” తో మార్చబడింది',
};
