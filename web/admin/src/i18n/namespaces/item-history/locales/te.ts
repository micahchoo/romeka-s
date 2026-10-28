/**
 * The Telugu item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Telugu. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const te: ItemHistoryMessages = {
  'itemHistory.loading': 'చరిత్రను చదవడం…',
  'itemHistory.empty': 'ఈ అంశంలో ఎటువంటి మార్పులు నమోదు కాలేదు.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'తొలగించబడిన వినియోగదారు',
  'itemHistory.source.bulk': 'బల్క్ మార్పు {id}',
  'itemHistory.source.import': 'ఇంపోర్ట్ {id}',
  'itemHistory.source.bulkUndo': 'బల్క్ మార్పు {id} యొక్క అన్డూ',
  'itemHistory.source.importUndo': 'ఇంపోర్ట్ {id} యొక్క అన్డూ',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'దృశ్యమానత',
  'itemHistory.field.class': 'తరగతి',
  'itemHistory.field.template': 'అంశం రకం',
  'itemHistory.field.itemSets': 'అంశాల సమితులు',
  'itemHistory.field.sites': 'సైట్లు',
  'itemHistory.public': 'బహిరంగ',
  'itemHistory.private': 'ప్రైవేటు',
  'itemHistory.none': '(ఏదీ లేదు)',
  'itemHistory.untitled': 'పేరు లేని {id}',
  'itemHistory.privateValue': '{value} (ప్రైవేటు)',
  'itemHistory.putBack': 'ఈ విలువను తిరిగి ఉంచండి',
  'itemHistory.puttingBack': 'ఈ విలువను తిరిగి ఉంచడం…',
  'itemHistory.putBackDone': 'విలువ తిరిగి వచ్చింది. పైన ఉన్న కొత్త మార్పు అదే చెబుతోంది.',
  'itemHistory.putBackSame': 'ఏమీ మారలేదు: ఐటెమ్ ఇప్పుడు ఈ విలువను కలిగి ఉంది.',
  'itemHistory.older': 'పాత మార్పులను చూపండి',
};
