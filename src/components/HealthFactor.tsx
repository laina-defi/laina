export const HEALTH_FACTOR_AUTO_THRESHOLD = 1.25;
export const HEALTH_FACTOR_MIN_THRESHOLD = 1.00;
export const HEALTH_FACTOR_GOOD_THRESHOLD = 1.20;
export const HEALTH_FACTOR_EXCELLENT_THRESHOLD = 1.40;

export const HealthFactor = ({ value }: { value: number }) => {
  if (value < HEALTH_FACTOR_MIN_THRESHOLD) {
    return <HealthBar text="Would liquidate immediately" textColor="text-red" bgColor="bg-red" bars={1} />;
  }
  if (value < HEALTH_FACTOR_GOOD_THRESHOLD) {
    return <HealthBar text="At risk of liquidation" textColor="text-yellow" bgColor="bg-yellow" bars={2} />;
  }
  if (value < HEALTH_FACTOR_EXCELLENT_THRESHOLD) {
    return <HealthBar text="Good" textColor="text-blue" bgColor="bg-blue" bars={3} />;
  }
  return <HealthBar text="Excellent" textColor="text-green" bgColor="bg-green" bars={4} />;
};

interface HealthBarProps {
  text: string;
  textColor: string;
  bgColor: string;
  bars: number;
}

const HealthBar = ({ text, textColor, bgColor, bars }: HealthBarProps) => (
  <>
    <p className={`${textColor} font-semibold transition-all`}>{text}</p>
    <div className="w-full flex flex-row gap-2">
      <div className={`transition-all h-3 w-full rounded-l ${bgColor}`} />
      <div className={`transition-all h-3 w-full ${bars > 1 ? bgColor : 'bg-grey'}`} />
      <div className={`transition-all h-3 w-full ${bars > 2 ? bgColor : 'bg-grey'}`} />
      <div className={`transition-all h-3 w-full rounded-r ${bars > 3 ? bgColor : 'bg-grey'}`} />
    </div>
  </>
);

export const CompactHealthFactor = ({ value }: { value: number }) => {
  if (value < HEALTH_FACTOR_MIN_THRESHOLD) {
    return (
      <div className="flex items-center">
        <div className="badge badge-error mr-2" />
        Liquidates
      </div>
    );
  }
  if (value < HEALTH_FACTOR_GOOD_THRESHOLD) {
    return (
      <div className="flex items-center">
        <div className="badge badge-warning mr-2" />
        At risk
      </div>
    );
  }
  if (value < HEALTH_FACTOR_EXCELLENT_THRESHOLD) {
    return (
      <div className="flex items-center">
        <div className="badge badge-info mr-2" />
        Good
      </div>
    );
  }
  return (
    <div className="flex items-center">
      <div className="badge badge-success mr-2" />
      Excellent
    </div>
  );
};
