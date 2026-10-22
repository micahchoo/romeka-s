/**
 * The places on the stage the notes panel draws in, so an editor marks the media
 * where it is shown rather than on a second copy beside it. The type lives here,
 * apart from the stage, so the panel's chunk names it without importing the
 * players.
 */
export interface StageSlots {
  /** Above the media: the drawing tools. */
  readonly tools: HTMLElement;
  /**
   * Exactly over the media: the shapes. When the panel draws the picture itself
   * (it claims the stage), this is where the picture goes, in the flow.
   */
  readonly over: HTMLElement;
  /** Under the media: the timeline and its fields. */
  readonly under: HTMLElement;
  /** The picture the stage shows, so the editor draws on the file already loaded. */
  readonly picture: StagePicture | null;
}

export interface StagePicture {
  readonly url: string;
  readonly svg: boolean;
  readonly alt: string;
}
