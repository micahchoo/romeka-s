# Recorded response fixtures

Each file below is the body of a real HTTP response, captured with `curl` on
2026-10-17 and stored unchanged. The tests serve them through a fake or a
loopback server, so no test touches the network.

| file | source URL | sha256 |
| --- | --- | --- |
| `oembed_response.json` | `https://www.youtube.com/oembed?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DdQw4w9WgXcQ&format=json` | `dc86b2c7c6a62d356a136f20df830a6656e122b80d735fcce9721d42bae70e3f` |
| `oembed_thumbnail.jpg` | `https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg` | `2fc77b4a0620f5dca2d3cae6d917af3acdeb3b2e5558ba1727f8d651351fc692` |
| `youtube_poster.jpg` | `http://img.youtube.com/vi/dQw4w9WgXcQ/0.jpg` | `2fc77b4a0620f5dca2d3cae6d917af3acdeb3b2e5558ba1727f8d651351fc692` |
| `url_response.png` | `https://iiif.io/img/logo-iiif-34x30.png` | `305a2471c2d7a587f847a59172bf2a4d29e5493c4b96391590bd117578ddc1dd` |
| `iiif_info_v2.json` | `https://iiif.bodleian.ox.ac.uk/iiif/image/9a2a5b7e-8b1a-4b6f-9c2e-000000000000/info.json` | `deb60a9c6a66251c403f1262d500121157b28eecb25b66fa5ec79adc32530422` |
| `iiif_presentation_v2.json` | `https://iiif.wellcomecollection.org/presentation/v2/b18035723` | `aa6f5088493246471fc8c5ffd62be1e34ca2c05f14266ac5f8ebf5a1d44dedc7` |

The `*_request.json` files are the ingest request bodies, shaped from the
upstream ingesters rather than captured, because a request body is built by the
port and has no server to record it from. `html_request.json` and
`upload_request.json` are request fixtures of the same kind.

The two YouTube captures are the same image (`hqdefault.jpg` and `0.jpg` are one
asset upstream), kept under both names because the two ingesters request
different URLs.
