-- omeka-db 0011_media_annotation_box_time (MySQL): a `rect` note may also hold a time range,
-- for a box on a video's frames over a span (`xywh=…&t=…`). The range keeps the `time` rule.
ALTER TABLE romeka_media_annotation DROP CHECK CK_ROMEKA_MEDIA_ANNOTATION_SHAPE;
ALTER TABLE romeka_media_annotation ADD CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_SHAPE CHECK (
       (selector_kind = 'whole' AND selector IS NULL AND t_start_ms IS NULL AND t_end_ms IS NULL)
    OR (selector_kind = 'polygon' AND selector IS NOT NULL
        AND t_start_ms IS NULL AND t_end_ms IS NULL)
    OR (selector_kind = 'rect' AND selector IS NOT NULL
        AND ((t_start_ms IS NULL AND t_end_ms IS NULL)
             OR (t_start_ms >= 0 AND t_end_ms > t_start_ms)))
    OR (selector_kind = 'time' AND selector IS NULL AND t_start_ms >= 0 AND t_end_ms > t_start_ms));
