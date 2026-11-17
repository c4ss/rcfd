// F-837: a toggle with an icon and no visible word (G22): its name is its accessible name and its tooltip, as
// IconButton's. A ruled toggle like Toggle ("Follow live"): deep ink with a white icon when on.
import { TooltipTrigger } from 'react-aria-components';
import { Toggle } from './Button.js';
import { Tooltip } from './Tooltip.js';
import type { IconName } from './pictogramPaths.js';

export function IconToggle({
  icon,
  label,
  isSelected,
  onChange,
  className,
  isDisabled,
}: {
  icon: IconName;
  label: string;
  isSelected: boolean;
  onChange: (v: boolean) => void;
  className?: string;
  isDisabled?: boolean;
}) {
  return (
    <TooltipTrigger delay={600}>
      <Toggle icon={icon} aria-label={label} isSelected={isSelected} onChange={onChange} className={className} isDisabled={isDisabled} />
      <Tooltip>{label}</Tooltip>
    </TooltipTrigger>
  );
}
