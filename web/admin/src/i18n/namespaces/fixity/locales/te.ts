/**
 * The Telugu fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Telugu. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const te: FixityMessages = {
  'fixity.class': 'నిల్వ చేసిన ఫైళ్లను తనిఖీ చేయండి',
  'fixity.start': 'నిల్వ చేసిన ఫైళ్లను తనిఖీ చేయండి',
  'fixity.start.note':
    'ప్రతి నిల్వ చేసిన అసలు ఫైల్‌ను చదివి, ఫైల్ జోడించినప్పుడు రికార్డ్ చేసిన SHA-256 తో పోల్చి చూస్తుంది.',
  'fixity.start.busy': 'తనిఖీ ప్రారంభిస్తోంది…',
  'fixity.start.failed': 'తనిఖీ ప్రారంభం కాలేదు: {message}',
  'fixity.result.title': 'ఫలితం',
  'fixity.result.loading': 'ఫలితాన్ని లోడ్ చేస్తోంది…',
  'fixity.result.failed': 'ఫలితాన్ని చదవలేకపోతోంది: {message}',
  'fixity.result.waiting': 'తనిఖీ ముగిసినప్పుడు ఇక్కడ ఫలితాన్ని చూపిస్తుంది.',
  'fixity.result.none':
    'ఈ తనిఖీ ఫలితాన్ని రాయకముందే ముగిసింది. దీని లాగ్ అది ఎందుకు ముగిసిందో చెబుతుంది.',
  'fixity.result.checkedOne': '{date} నాడు 1 ఫైల్ తనిఖీ చేయబడింది.',
  'fixity.result.checkedMany': '{count} ఫైళ్లు {date} నాడు తనిఖీ చేయబడ్డాయి.',
  'fixity.result.differsOne': '1 భిన్నంగా ఉంది.',
  'fixity.result.differsMany': '{count} భిన్నంగా ఉన్నాయి.',
  'fixity.result.missingOne': '1 లోపించింది.',
  'fixity.result.missingMany': '{count} లోపించాయి.',
  'fixity.result.unreadableOne': '1 చదవడానికి వీలుపడదు.',
  'fixity.result.unreadableMany': '{count} చదవడానికి వీలుపడవు.',
  'fixity.result.allMatch': 'అన్ని సరిపోలాయి.',
  'fixity.result.stopped': 'చివరి ఫైల్ కంటే ముందే తనిఖీ నిలిపివేయబడింది.',
  'fixity.line.problemsOne': 'చివరిగా తనిఖీ చేసిన ఫైళ్లు {date}: 1 సమస్య',
  'fixity.line.problemsMany': 'చివరిగా తనిఖీ చేసిన ఫైళ్లు {date}: {count} సమస్యలు',
  'fixity.line.noProblems': 'చివరిగా తనిఖీ చేసిన ఫైళ్లు {date}: ఎటువంటి సమస్యలు లేవు',
  'fixity.line.never': 'నిల్వ చేసిన ఫైళ్లను ఎప్పుడూ తనిఖీ చేయలేదు.',
  'fixity.line.running': 'నిల్వ చేసిన ఫైళ్లను ఇప్పుడు తనిఖీ చేస్తున్నాను.',
  'fixity.filter.checked': '#{job} తనిఖీలో చదివిన ఫైళ్లు.',
  'fixity.filter.ok': '#{job} తనిఖీలో సరిపోలిన ఫైళ్లు.',
  'fixity.filter.differs': '#{job} తనిఖీలో తేడాలు ఉన్న ఫైళ్లు.',
  'fixity.filter.missing': '#{job} తనిఖీలో లేని ఫైళ్లు.',
  'fixity.filter.unreadable': '#{job} తనిఖీలో చదవలేని ఫైళ్లు.',
  'fixity.filter.back': 'తనిఖీని తెరవండి',
  'fixity.filter.clear': 'అన్ని మీడియాను చూపండి',
};
