/**
 * The Nepali words of the Structure screen's corrections.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, then read against English and corrected by hand, and not yet
 * reviewed by a reader of Nepali. Correct a string here; a later run keeps it. It is
 * typed `ImportFixesMessages`, so a key added to English and not here is a `tsc` error.
 */
import type { ImportFixesMessages } from "../index";

export const ne: ImportFixesMessages = {
  'fix.q.items': 'प्रस्ताव: वस्तुहरू',
  'fix.q.rows': 'प्रस्ताव: पङ्क्तिहरू',
  'fix.oneItem': '{count} वस्तुहरू अर्को वस्तुभित्र जान सक्छन्।',
  'fix.oneItemOne': '1 वस्तु अर्को वस्तुभित्र जान सक्छ।',
  'fix.rowIs': '{count} पङ्क्तिले कुनै फाइलको नाम राख्दैनन्।',
  'fix.rowIsOne': '1 पङ्क्तिले कुनै फाइलको नाम राख्दैन।',
  'fix.rowIs.note': 'प्रत्येकले कुनै पङ्क्ति नभएको एउटा फाइललाई जनाउँछ।',
  'fix.rowIs.noteOne': 'यसले कुनै पङ्क्ति नभएको एउटा फाइललाई जनाउँछ।',
  'fix.describesNothing':
    '{count} पङ्क्तिले कुनै फाइलको नाम राख्दैनन्, र कुनै फाइल नामले नजिक छैन।',
  'fix.describesNothingOne': '1 पङ्क्तिले कुनै फाइलको नाम राख्दैन, र कुनै फाइल नामले नजिक छैन।',
  'fix.describesNothing.note': 'प्रत्येकले फाइल नभएको एउटा वस्तु बनाउँछ।',
  'fix.describesNothing.noteOne': 'यसले फाइल नभएको एउटा वस्तु बनाउँछ।',
  'fix.items': 'वस्तुहरू: {before} → {after}',
  'fix.rows': 'फाइल फेला पार्ने पङ्क्तिहरू: {of} मध्ये {before} → {after}',
  'fix.show': '{count} देखाउनुहोस्',
  'fix.showOne': 'यसलाई देखाउनुहोस्',
  'fix.oneItem.take': 'यी {count} लाई तिनको वस्तुमा राख्नुहोस्',
  'fix.oneItem.takeOne': 'यसलाई यसको वस्तुमा राख्नुहोस्',
  'fix.rowIs.take': 'यी {count} को जोडी मिलाउनुहोस्',
  'fix.rowIs.takeOne': 'यो पङ्क्तिको जोडी मिलाउनुहोस्',
  'fix.describesNothing.take': 'यी {count} छोड्नुहोस्',
  'fix.describesNothing.takeOne': 'यो पङ्क्ति छोड्नुहोस्',
  'fix.join': '{with} {item} को वस्तुमा जान्छ',
  'fix.pair': 'पङ्क्ति {line}, {cell}: फाइल {file}',
  'fix.leave': 'पङ्क्ति {line}, {cell}',
  'fix.because.keyExtends': 'यसको नाम अर्को वस्तुको नामबाट सुरु हुन्छ।',
  'fix.because.rowNamesBoth': 'एउटा पङ्क्तिले दुवैको नाम दिन्छ।',
  'fix.because.near': 'नामहरूमा एउटा अक्षरको फरक छ।',
  'fix.because.namesItem': 'पङ्क्तिले यो फाइलको वस्तुको नाम दिन्छ।',
  'fix.because.namesPart': 'पङ्क्तिले आफ्नो वस्तुमा यो फाइलको भागको नाम दिन्छ।',
  'fix.joined': 'अर्को वस्तुमा: {count}',
  'fix.paired': 'जोडी मिलाइएका: {count}',
  'fix.left': 'छोडिएका: {count}',
  'fix.stale': '{count} अब लागू हुँदैनन्।',
  'fix.staleOne': '1 अब लागू हुँदैन।',
  'fix.rowChanged': 'यो पङ्क्ति परिवर्तन भयो।',
  'fix.fileGone': 'यो फाइल अब छैन।',
};
