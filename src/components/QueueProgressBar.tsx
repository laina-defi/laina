const FOURTEEN_DAYS_IN_SECONDS = 14 * 24 * 60 * 60;

export const QueueProgressBar = ({ queuedAtTimestamp }: { queuedAtTimestamp: bigint }) => {
  const nowSeconds = Date.now() / 1000;
  const elapsedSeconds = nowSeconds - Number(queuedAtTimestamp);
  const elapsedDays = elapsedSeconds / (24 * 60 * 60);
  const remainingDays = Math.max(0, 14 - elapsedDays);

  if (elapsedSeconds >= FOURTEEN_DAYS_IN_SECONDS) {
    return <QueueBar text="Ready to withdraw!" textColor="text-green" bgColor="bg-green" bars={4} />;
  }
  if (elapsedDays >= 10.5) {
    return (
      <QueueBar
        text={`Day ${Math.floor(elapsedDays)} of 14 – ${Math.ceil(remainingDays)} days remaining`}
        textColor="text-blue"
        bgColor="bg-blue"
        bars={3}
      />
    );
  }
  if (elapsedDays >= 7) {
    return (
      <QueueBar
        text={`Day ${Math.floor(elapsedDays)} of 14 – ${Math.ceil(remainingDays)} days remaining`}
        textColor="text-yellow"
        bgColor="bg-yellow"
        bars={2}
      />
    );
  }
  return (
    <QueueBar
      text={`Day ${Math.floor(elapsedDays)} of 14 – ${Math.ceil(remainingDays)} days remaining`}
      textColor="text-red"
      bgColor="bg-red"
      bars={1}
    />
  );
};

interface QueueBarProps {
  text: string;
  textColor: string;
  bgColor: string;
  bars: number;
}

const QueueBar = ({ text, textColor, bgColor, bars }: QueueBarProps) => (
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
