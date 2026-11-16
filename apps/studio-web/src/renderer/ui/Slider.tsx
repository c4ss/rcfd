// Slider. DESIGN.md draws one slider, the scrubber (a deep ink line that turns orange while dragged, F-810); this is the
// general control on the same terms: a 1px ink track, a 3px ink fill, a square ink thumb that turns orange while
// dragged, the value beside the label in tabular figures. Arrows step; Home and End go to the limits.
import {
  Label,
  Slider as AriaSlider,
  SliderFill,
  SliderOutput,
  SliderThumb,
  SliderTrack,
  type SliderProps as AriaSliderProps,
} from 'react-aria-components';
import { cx } from './cx.js';

export interface SliderProps extends Omit<AriaSliderProps<number>, 'className' | 'style' | 'children'> {
  /** The visible label, with the unit when there is one ("Position, %"). */
  label: string;
  className?: string;
}

export function Slider({ label, className, ...rest }: SliderProps) {
  return (
    <AriaSlider {...rest} className={cx('ui-slider', className)}>
      <div className="ui-slider-head">
        <Label className="ui-label">{label}</Label>
        <SliderOutput className="ui-slider-output" />
      </div>
      <SliderTrack className="ui-slider-track">
        <span className="ui-slider-rail" />
        {/* SliderFill sizes itself to its parent's height, so the 3px line is its parent. */}
        <span className="ui-slider-line">
          <SliderFill className="ui-slider-fill" />
        </span>
        <SliderThumb className="ui-slider-thumb" />
      </SliderTrack>
    </AriaSlider>
  );
}
