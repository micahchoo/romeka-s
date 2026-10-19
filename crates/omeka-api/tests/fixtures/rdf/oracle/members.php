<?php
// Usage: php members.php <file> <namespace> <format> [lang] [label_property] [comment_property]
// Prints RdfImporter::getMembers('file', ...) as JSON, in upstream's key order.
require '/var/www/html/vendor/autoload.php';
$importer = (new ReflectionClass(Omeka\Stdlib\RdfImporter::class))->newInstanceWithoutConstructor();
$options = ['file' => $argv[1], 'format' => $argv[3]];
if (isset($argv[4]) && $argv[4] !== '') $options['lang'] = $argv[4];
if (isset($argv[5]) && $argv[5] !== '') $options['label_property'] = $argv[5];
if (isset($argv[6]) && $argv[6] !== '') $options['comment_property'] = $argv[6];
try {
    $members = $importer->getMembers('file', $argv[2], $options);
    $out = [];
    foreach (['classes', 'properties'] as $kind) {
        $out[$kind] = [];
        foreach ($members[$kind] as $localName => $info) {
            $out[$kind][] = ['local_name' => (string) $localName, 'label' => $info['label'], 'comment' => $info['comment']];
        }
    }
    echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE), "\n";
} catch (\Throwable $e) {
    echo json_encode(['error' => get_class($e), 'message' => $e->getMessage()], JSON_UNESCAPED_SLASHES), "\n";
}
