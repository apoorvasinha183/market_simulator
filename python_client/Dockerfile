# Use a Miniconda base image
FROM continuumio/miniconda3

WORKDIR /usr/src/app

# Copy environment definition and application code
COPY environment.yml .
COPY . .

# Create the Conda environment
RUN conda env create -f environment.yml

# Set the shell to bash to activate the conda environment
SHELL ["/bin/bash", "-c"]

# Define the command to run the application
CMD ["conda run -n market_client python -u run_broker.py"]