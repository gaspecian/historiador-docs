const DATE_FMT = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short" });
const DATETIME_FMT = new Intl.DateTimeFormat("pt-BR", {
  dateStyle: "short",
  timeStyle: "short",
});
const TIME_FMT = new Intl.DateTimeFormat("pt-BR", { timeStyle: "short" });
const NUMBER_FMT = new Intl.NumberFormat("pt-BR");

export const formatDate = (d: Date | string | number): string =>
  DATE_FMT.format(new Date(d));

export const formatDateTime = (d: Date | string | number): string =>
  DATETIME_FMT.format(new Date(d));

export const formatTime = (d: Date | string | number): string =>
  TIME_FMT.format(new Date(d));

export const formatNumber = (n: number): string => NUMBER_FMT.format(n);
