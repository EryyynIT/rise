/* Memory layout — placeholder for the STM32F401CC (256K flash, 64K RAM).
   Adjust ORIGIN/LENGTH to your MCU's datasheet. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 256K
  RAM   : ORIGIN = 0x20000000, LENGTH = 64K
}
